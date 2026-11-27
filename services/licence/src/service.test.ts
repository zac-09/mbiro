import {
  createHash,
  generateKeyPairSync,
  verify as verifySignature,
  createPublicKey,
} from "node:crypto";
import { runInNewContext } from "node:vm";
import { describe, expect, it } from "vitest";
import { issueKey, keyId, loadSigner, readKey } from "./keys";
import type { Message } from "./mail";
import { STUB_SIGNATURE_HEADER, stubAdapter, stubSignature } from "./payments";
import { DEVICE_LIMIT, createService } from "./service";
import { MemoryStore } from "./store";

/** A key pair that exists only for one test. It signs nothing real. */
const throwaway = () =>
  generateKeyPairSync("ed25519").privateKey.export({ format: "pem", type: "pkcs8" }).toString();

const SECRET = "a-secret-for-tests-only";
const device = (n: number) => n.toString(16).padStart(64, "0");

function setUp(options: { mailFails?: boolean; publicUrl?: string } = {}) {
  const signer = loadSigner(throwaway());
  const sent: Message[] = [];
  const state = { mailFails: options.mailFails ?? false, now: new Date("2026-11-23T10:00:00Z") };
  const service = createService({
    signer,
    store: new MemoryStore(),
    mailer: {
      send(message) {
        if (state.mailFails) return Promise.reject(new Error("mail is down"));
        sent.push(message);
        return Promise.resolve();
      },
    },
    adapters: [stubAdapter(SECRET)],
    publicUrl: options.publicUrl,
    now: () => state.now,
  });
  const post = async (path: string, body: unknown, headers: Record<string, string> = {}) => {
    const response = await service(
      new Request(`https://licence.test${path}`, {
        method: "POST",
        headers,
        body: typeof body === "string" ? body : JSON.stringify(body),
      }),
    );
    return { status: response.status, body: (await response.json()) as Record<string, unknown> };
  };
  const webhook = (event: unknown, secret = SECRET) => {
    const body = JSON.stringify(event);
    return post("/v1/payments/stub", body, {
      [STUB_SIGNATURE_HEADER]: stubSignature(secret, body),
    });
  };
  const revoked = async () => {
    const response = await service(new Request("https://licence.test/v1/revoked"));
    return ((await response.json()) as { revoked: string[] }).revoked;
  };
  /** The key in the most recent licence email. */
  const emailedKey = () => {
    const key = sent
      .at(-1)
      ?.text.split("\n")
      .find((line) => /^[\w-]{40,}\.[\w-]{80,}$/.test(line));
    if (!key) throw new Error("no key in the last email");
    return key;
  };
  const key = issueKey(signer, {
    email: "buyer@example.com",
    plan: "standard",
    issued: "2026-11-23",
  });
  return { signer, service, post, webhook, revoked, emailedKey, sent, state, key };
}

describe("keys", () => {
  it("makes keys in the form the app reads", () => {
    const pem = throwaway();
    const signer = loadSigner(pem);
    const key = issueKey(signer, { email: "a@b.co", plan: "standard", issued: "2026-11-23" });
    const [first = "", second = ""] = key.split(".");
    const payload = Buffer.from(first, "base64url");
    const signature = Buffer.from(second, "base64url");
    // Exactly the bytes scripts/issue-licence.sh signs, in the same field order.
    expect(payload.toString()).toBe('{"email":"a@b.co","plan":"standard","issued":"2026-11-23"}');
    expect(signature).toHaveLength(64);
    expect(verifySignature(null, payload, createPublicKey(pem), signature)).toBe(true);
    expect(Buffer.from(signer.publicKey, "base64")).toHaveLength(32);
  });

  it("reads its own keys and nothing else", () => {
    const signer = loadSigner(throwaway());
    const key = issueKey(signer, { email: "a@b.co", plan: "standard", issued: "2026-11-23" });
    expect(readKey(signer, ` ${key.slice(0, 30)}\n${key.slice(30)} `)?.email).toBe("a@b.co");
    expect(readKey(loadSigner(throwaway()), key)).toBeNull();
    expect(
      readKey(
        signer,
        key.replace(/^./, (c) => (c === "A" ? "B" : "A")),
      ),
    ).toBeNull();
    expect(readKey(signer, "not a key")).toBeNull();
    // Signed by us, but not a licence.
    expect(readKey(signer, signer.sign('{"licence":"x","device":"y"}'))).toBeNull();
    // Wrapped and unwrapped, a key has one id.
    expect(keyId(`${key.slice(0, 30)}\n${key.slice(30)}`)).toBe(keyId(key));
  });

  it("refuses a private key of another kind", () => {
    const rsa = generateKeyPairSync("rsa", { modulusLength: 2048 }).privateKey;
    expect(() => loadSigner(rsa.export({ format: "pem", type: "pkcs8" }).toString())).toThrow();
  });
});

describe("activation", () => {
  it("gives a Mac a signed receipt naming the licence and the Mac", async () => {
    const { post, key, signer } = setUp();
    const { status, body } = await post("/v1/activate", { key, device: device(1) });
    expect(status).toBe(200);
    const payload = signer.verify(body.receipt as string);
    expect(payload).toBe(JSON.stringify({ licence: keyId(key), device: device(1) }));
  });

  it("allows three Macs and refuses a fourth until one is removed", async () => {
    const { post, key } = setUp();
    for (let n = 1; n <= DEVICE_LIMIT; n++) {
      expect((await post("/v1/activate", { key, device: device(n) })).status).toBe(200);
    }
    // A Mac that already has a place can ask again.
    expect((await post("/v1/activate", { key, device: device(2) })).status).toBe(200);
    const fourth = await post("/v1/activate", { key, device: device(4) });
    expect(fourth).toEqual({ status: 409, body: { ok: false, code: "device_limit" } });

    expect((await post("/v1/deactivate", { key, device: device(1) })).status).toBe(200);
    expect((await post("/v1/activate", { key, device: device(4) })).status).toBe(200);
    expect((await post("/v1/activate", { key, device: device(1) })).status).toBe(409);
  });

  it("refuses keys it did not sign, and requests that are not requests", async () => {
    const { post, key } = setUp();
    const foreign = issueKey(loadSigner(throwaway()), {
      email: "thief@example.com",
      plan: "standard",
      issued: "2026-11-23",
    });
    const refused = { status: 403, body: { ok: false, code: "invalid_key" } };
    expect(await post("/v1/activate", { key: foreign, device: device(1) })).toEqual(refused);
    expect(await post("/v1/deactivate", { key: foreign, device: device(1) })).toEqual(refused);
    for (const bad of [
      "not json",
      {},
      { key },
      { key, device: "my-mac" },
      { key, device: device(1), extra: true },
      { key: "x".repeat(20_000), device: device(1) },
    ]) {
      expect((await post("/v1/activate", bad)).status).toBe(400);
    }
  });

  it("answers 404 for anything else", async () => {
    const { service } = setUp();
    for (const [method, path] of [
      ["GET", "/"],
      ["GET", "/v1/activate"],
      ["POST", "/v1/revoked"],
      ["POST", "/v1/payments/unknown"],
    ] as const) {
      expect((await service(new Request(`https://licence.test${path}`, { method }))).status).toBe(
        404,
      );
    }
  });
});

describe("purchases and refunds", () => {
  const bought = { kind: "purchase", order: "1001", email: "buyer@example.com", plan: "standard" };

  it("emails a key that activates", async () => {
    const { webhook, post, sent, emailedKey, signer } = setUp();
    expect((await webhook(bought)).status).toBe(200);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.to).toBe("buyer@example.com");
    expect(readKey(signer, emailedKey())).toEqual({
      email: "buyer@example.com",
      plan: "standard",
      issued: "2026-11-23",
    });
    expect((await post("/v1/activate", { key: emailedKey(), device: device(1) })).status).toBe(200);
  });

  it("does not issue or email twice when the provider repeats itself", async () => {
    const { webhook, sent, emailedKey, state } = setUp();
    await webhook(bought);
    const first = emailedKey();
    state.now = new Date("2026-11-25T10:00:00Z");
    expect((await webhook(bought)).status).toBe(200);
    expect(sent).toHaveLength(1);
    expect(emailedKey()).toBe(first);
  });

  it("sends the same key on the retry when the first email failed", async () => {
    const { webhook, sent, emailedKey, state, signer } = setUp({ mailFails: true });
    expect((await webhook(bought)).status).toBe(502);
    expect(sent).toHaveLength(0);
    state.mailFails = false;
    state.now = new Date("2026-11-24T10:00:00Z");
    expect((await webhook(bought)).status).toBe(200);
    expect(sent).toHaveLength(1);
    // The key kept from the first attempt, not a second one.
    expect(readKey(signer, emailedKey())?.issued).toBe("2026-11-23");
  });

  it("lists a refunded licence and stops it activating", async () => {
    const { webhook, post, revoked, emailedKey } = setUp();
    await webhook(bought);
    await webhook({ ...bought, order: "1002", email: "other@example.com" });
    const other = emailedKey();
    expect(await revoked()).toEqual([]);

    expect((await webhook({ kind: "refund", order: "1002" })).status).toBe(200);
    expect(await revoked()).toEqual([keyId(other)]);
    expect(await post("/v1/activate", { key: other, device: device(1) })).toEqual({
      status: 403,
      body: { ok: false, code: "revoked" },
    });
    expect((await webhook({ kind: "refund", order: "9999" })).status).toBe(404);
  });

  it("refuses a webhook that is not signed with the secret", async () => {
    const { webhook, post, sent } = setUp();
    expect((await webhook(bought, "some-other-secret-value")).status).toBe(401);
    expect((await post("/v1/payments/stub", bought)).status).toBe(401);
    expect((await webhook({ kind: "purchase", order: "1", email: "not an email" })).status).toBe(
      401,
    );
    expect(sent).toHaveLength(0);
  });
});

describe("the activation link", () => {
  const purchase = { kind: "purchase", order: "1", email: "buyer@example.com", plan: "standard" };

  it("is in the email when the service knows its address, with the key after the #", async () => {
    const { webhook, emailedKey, sent } = setUp({ publicUrl: "https://licence.test/" });
    await webhook(purchase);
    const key = emailedKey();
    expect(sent.at(-1)?.text.split("\n")).toContain(`https://licence.test/activate#${key}`);
  });

  it("is left out when the service does not know its address", async () => {
    const { webhook, emailedKey, sent } = setUp();
    await webhook(purchase);
    expect(emailedKey()).toBeTruthy();
    expect(sent.at(-1)?.text).not.toContain("/activate");
  });

  it("leads to a page that hands a key-shaped fragment to the app and runs nothing else", async () => {
    const { service } = setUp();
    const response = await service(new Request("https://licence.test/activate"));
    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toContain("text/html");
    expect(response.headers.get("referrer-policy")).toBe("no-referrer");
    const page = await response.text();
    expect(page).toContain('"mbiro://activate?key="');

    // The policy allows exactly the page's own script and style.
    const policy = response.headers.get("content-security-policy") ?? "";
    const inline = (tag: string) =>
      new RegExp(`<${tag}>([\\s\\S]*?)</${tag}>`).exec(page)?.[1] ?? "";
    for (const tag of ["script", "style"]) {
      const digest = createHash("sha256").update(inline(tag)).digest("base64");
      expect(policy).toContain(`${tag}-src 'sha256-${digest}'`);
    }
    expect(policy).toContain("default-src 'none'");
  });

  it("page script: passes on a key, and nothing that is not shaped like one", async () => {
    const { service } = setUp();
    const page = await (await service(new Request("https://licence.test/activate"))).text();
    const script = /<script>([\s\S]*?)<\/script>/.exec(page)?.[1] ?? "";
    /** Runs the page's script against a stand-in for the browser. */
    const open = (fragment: string) => {
      const elements: Record<string, { hidden: boolean; href?: string }> = {
        found: { hidden: true },
        missing: { hidden: true },
        open: { hidden: false },
      };
      const location = { hash: fragment, pathname: "/activate", href: "" };
      const bar: string[] = [];
      runInNewContext(script, {
        location,
        document: { getElementById: (id: string) => elements[id] },
        history: { replaceState: (_: unknown, __: string, to: string) => bar.push(to) },
      });
      return { elements, location, bar };
    };

    const good = open("#eyJ_a-1.c2ln");
    expect(good.location.href).toBe("mbiro://activate?key=eyJ_a-1.c2ln");
    expect(good.elements.open?.href).toBe("mbiro://activate?key=eyJ_a-1.c2ln");
    expect(good.elements.found?.hidden).toBe(false);
    // The key is taken out of the address bar.
    expect(good.bar).toEqual(["/activate"]);

    for (const fragment of ["", "#", "#nodot", "#a.b.c", "#a b.c", "#<img src=x>.c", "#a.b&x=1"]) {
      const bad = open(fragment);
      expect(bad.location.href).toBe("");
      expect(bad.elements.missing?.hidden).toBe(false);
      expect(bad.elements.found?.hidden).toBe(true);
    }
  });
});
