/**
 * The licence service, as one function from a request to a reply. It knows
 * nothing about where it runs: the host hands it requests, a store, a mailer
 * and the payment adapters.
 *
 *   POST /v1/activate            {key, device}  -> {ok, receipt}
 *   POST /v1/deactivate          {key, device}  -> {ok}
 *   GET  /v1/revoked                            -> {revoked: [licence id]}
 *   POST /v1/payments/<provider>  a provider's webhook
 *
 * The app's side of the first three is apps/desktop/src-tauri/src/activation.rs.
 * Nothing here logs a key, an email address or a device.
 */
import { z } from "zod";
import { issueKey, issueReceipt, keyId, readKey, type Signer } from "./keys";
import type { Mailer } from "./mail";
import type { PaymentAdapter, PaymentEvent } from "./payments";
import type { Store } from "./store";
import { strings } from "./strings";

/** Macs one licence may run on at a time (PLAN.md section 8). */
export const DEVICE_LIMIT = 3;
const MAX_BODY_BYTES = 16 * 1024;

export interface ServiceOptions {
  signer: Signer;
  store: Store;
  mailer: Mailer;
  adapters: PaymentAdapter[];
  /** The clock, so tests can set it. */
  now?: () => Date;
}

/** Why a request was refused, as the app reads it. */
export type RefusalCode =
  | "bad_request"
  | "invalid_key"
  | "revoked"
  | "device_limit"
  | "not_found"
  | "unauthorised"
  | "failed";

const activationRequest = z.strictObject({
  key: z.string().min(1).max(2048),
  /** A hash made by the app. The service never sees a hardware identifier. */
  device: z.string().regex(/^[0-9a-f]{64}$/),
});

const reply = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store" },
  });
const refuse = (status: number, code: RefusalCode) => reply(status, { ok: false, code });

/** Reads the body as text, refusing anything too large to be one of ours. */
async function bodyText(request: Request): Promise<string | null> {
  const bytes = new Uint8Array(await request.arrayBuffer());
  if (bytes.length > MAX_BODY_BYTES) return null;
  return new TextDecoder().decode(bytes);
}

export function createService(options: ServiceOptions): (request: Request) => Promise<Response> {
  const { signer, store, mailer } = options;
  const now = options.now ?? (() => new Date());
  const adapters = new Map(options.adapters.map((adapter) => [adapter.name, adapter]));

  /** Parses an activation request and checks its key is ours. */
  async function licensed(request: Request) {
    const text = await bodyText(request);
    let parsed;
    try {
      parsed = activationRequest.safeParse(JSON.parse(text ?? ""));
    } catch {
      return refuse(400, "bad_request");
    }
    if (!parsed.success) return refuse(400, "bad_request");
    if (!readKey(signer, parsed.data.key)) return refuse(403, "invalid_key");
    return { licence: keyId(parsed.data.key), device: parsed.data.device };
  }

  async function activate(request: Request): Promise<Response> {
    const asked = await licensed(request);
    if (asked instanceof Response) return asked;
    const outcome = await store.activate(asked.licence, asked.device, DEVICE_LIMIT, now());
    if (outcome === "revoked") return refuse(403, "revoked");
    if (outcome === "device_limit") return refuse(409, "device_limit");
    return reply(200, { ok: true, receipt: issueReceipt(signer, asked) });
  }

  async function deactivate(request: Request): Promise<Response> {
    // Only someone holding the key can free one of its places.
    const asked = await licensed(request);
    if (asked instanceof Response) return asked;
    await store.deactivate(asked.licence, asked.device);
    return reply(200, { ok: true });
  }

  async function purchase(
    provider: string,
    event: Extract<PaymentEvent, { kind: "purchase" }>,
  ): Promise<Response> {
    const ref = `${provider}:${event.order}`;
    const licence = {
      email: event.email,
      plan: event.plan,
      issued: now().toISOString().slice(0, 10),
    };
    const key = issueKey(signer, licence);
    // Providers repeat webhooks. A repeat keeps the first key, and sends the
    // email only if the first attempt did not.
    const order = await store.keepOrder(ref, {
      key,
      licence: keyId(key),
      email: event.email,
      emailed: false,
    });
    if (!order.emailed) {
      try {
        await mailer.send({
          to: order.email,
          subject: strings.licenceEmail.subject,
          text: strings.licenceEmail.body(order.key, licence, DEVICE_LIMIT),
        });
      } catch {
        // An error status makes the provider try again later.
        return refuse(502, "failed");
      }
      await store.markEmailed(ref);
    }
    return reply(200, { ok: true });
  }

  async function payment(provider: string, request: Request): Promise<Response> {
    const adapter = adapters.get(provider);
    if (!adapter) return refuse(404, "not_found");
    const text = await bodyText(request);
    if (text === null) return refuse(400, "bad_request");
    const event = await adapter.read(request.headers, text);
    if (event === null) return refuse(401, "unauthorised");
    if (event === "ignored") return reply(200, { ok: true });
    if (event.kind === "purchase") return purchase(provider, event);
    const known = await store.revokeOrder(`${provider}:${event.order}`);
    return known ? reply(200, { ok: true }) : refuse(404, "not_found");
  }

  return async (request) => {
    const { pathname } = new URL(request.url);
    try {
      if (request.method === "POST" && pathname === "/v1/activate") return await activate(request);
      if (request.method === "POST" && pathname === "/v1/deactivate") {
        return await deactivate(request);
      }
      if (request.method === "GET" && pathname === "/v1/revoked") {
        return reply(200, { revoked: await store.revoked() });
      }
      const provider = /^\/v1\/payments\/([a-z0-9-]{1,32})$/.exec(pathname)?.[1];
      if (request.method === "POST" && provider) return await payment(provider, request);
      return refuse(404, "not_found");
    } catch {
      // Whatever went wrong may involve a key, so it is not passed on or logged here.
      return refuse(500, "failed");
    }
  };
}
