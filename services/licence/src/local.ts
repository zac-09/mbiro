/**
 * Runs the service on this machine, for trying the app against it:
 *
 *   LICENCE_PRIVATE_KEY_FILE=/somewhere/safe/licence-private.pem \
 *   STUB_PAYMENT_SECRET=<16 characters or more> \
 *   pnpm --filter @mbiro/licence-service dev
 *
 * It listens on 127.0.0.1 only, keeps everything in memory and forgets it on
 * exit, and prints each licence email instead of sending it. It is not what
 * gets deployed: the hosted version needs a real store and a real mailer.
 */
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { loadSigner } from "./keys";
import type { Mailer } from "./mail";
import { stubAdapter } from "./payments";
import { createService } from "./service";
import { MemoryStore } from "./store";

const need = (name: string): string => {
  const value = process.env[name];
  if (!value) {
    console.error(`error: set ${name} (see the top of services/licence/src/local.ts)`);
    process.exit(1);
  }
  return value;
};

const signer = loadSigner(readFileSync(need("LICENCE_PRIVATE_KEY_FILE"), "utf8"));
const printer: Mailer = {
  send(message) {
    console.log(`\n--- email to ${message.to}: ${message.subject}\n${message.text}\n---`);
    return Promise.resolve();
  },
};
const service = createService({
  signer,
  store: new MemoryStore(),
  mailer: printer,
  adapters: [stubAdapter(need("STUB_PAYMENT_SECRET"))],
});

const port = Number(process.env.PORT ?? 8787);
createServer((incoming, outgoing) => {
  const chunks: Buffer[] = [];
  incoming.on("data", (chunk: Buffer) => chunks.push(chunk));
  incoming.on("end", () => {
    const headers = new Headers();
    for (const [name, value] of Object.entries(incoming.headers)) {
      if (typeof value === "string") headers.set(name, value);
    }
    const method = incoming.method ?? "GET";
    const request = new Request(`http://127.0.0.1:${port}${incoming.url ?? "/"}`, {
      method,
      headers,
      body: method === "GET" || method === "HEAD" ? undefined : Buffer.concat(chunks),
    });
    void service(request).then(async (response) => {
      outgoing.writeHead(response.status, Object.fromEntries(response.headers));
      outgoing.end(await response.text());
    });
  });
}).listen(port, "127.0.0.1", () => {
  console.log(`Licence service on http://127.0.0.1:${port} (local, in memory)`);
  console.log(`Public key for the app: ${signer.publicKey}`);
});
