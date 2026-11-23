/**
 * Payment providers, each behind one adapter. An adapter's only job is to
 * prove a webhook came from its provider and say what happened.
 *
 * The provider is not chosen yet, so the only adapter here is `stubAdapter`,
 * for tests, local runs and issuing by hand.
 */
import { createHmac, timingSafeEqual } from "node:crypto";
import { z } from "zod";

export type PaymentEvent =
  | { kind: "purchase"; order: string; email: string; plan: string }
  | { kind: "refund"; order: string };

export interface PaymentAdapter {
  /** The name in the webhook's path: `/v1/payments/<name>`. */
  name: string;
  /**
   * Returns the event if the request is genuine, `"ignored"` for a genuine
   * event that is neither a purchase nor a refund, and nothing if it is not
   * genuine. `body` is the exact text received, as signatures cover it.
   */
  read(headers: Headers, body: string): Promise<PaymentEvent | "ignored" | null>;
}

const stubEvent = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("purchase"),
    order: z.string().min(1).max(128),
    email: z.email().max(320),
    plan: z
      .string()
      .regex(/^[A-Za-z0-9_-]{1,32}$/)
      .default("standard"),
  }),
  z.strictObject({ kind: z.literal("refund"), order: z.string().min(1).max(128) }),
]);

export const STUB_SIGNATURE_HEADER = "x-stub-signature";

/** What the stub expects in its signature header: hex HMAC-SHA256 of the body. */
export function stubSignature(secret: string, body: string): string {
  return createHmac("sha256", secret).update(body, "utf8").digest("hex");
}

/**
 * Stands in for a provider. A request counts as genuine when it carries the
 * HMAC of its body under a secret only the owner holds.
 */
export function stubAdapter(secret: string): PaymentAdapter {
  if (secret.length < 16) throw new Error("the stub payment secret must be 16 characters or more");
  return {
    name: "stub",
    read(headers, body) {
      const given = Buffer.from(headers.get(STUB_SIGNATURE_HEADER) ?? "", "utf8");
      const wanted = Buffer.from(stubSignature(secret, body), "utf8");
      if (given.length !== wanted.length || !timingSafeEqual(given, wanted)) {
        return Promise.resolve(null);
      }
      try {
        return Promise.resolve(stubEvent.parse(JSON.parse(body)));
      } catch {
        return Promise.resolve(null);
      }
    },
  };
}
