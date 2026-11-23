/**
 * Licence keys and activation receipts: making them and checking them.
 *
 * Both are `<payload>.<signature>`, each part URL-safe base64 without padding,
 * the signature an Ed25519 signature of the payload bytes. The app checks them
 * with the public key in apps/desktop/src-tauri/src/licence.rs; the two files
 * must agree on every field name.
 */
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  sign,
  verify,
  type KeyObject,
} from "node:crypto";
import { z } from "zod";

/** What a licence key says. Field order is the order in the signed payload. */
export const licenceSchema = z.strictObject({
  email: z.string().min(3).max(320),
  plan: z.string().min(1).max(64),
  /** The day it was issued, `YYYY-MM-DD`. */
  issued: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
});
export type Licence = z.infer<typeof licenceSchema>;

/** What an activation receipt says: this licence may run on this Mac. */
export interface Receipt {
  /** The licence's id: see `keyId`. */
  licence: string;
  device: string;
}

/** Holds the private key. Made once at start-up and passed around; never logged. */
export interface Signer {
  sign(payload: string): string;
  verify(signed: string): string | null;
  /** The public half as the app wants it: standard base64 of its 32 bytes. */
  publicKey: string;
}

/** Reads the private key from its PEM text (PKCS#8, as `licence-keygen.sh` writes it). */
export function loadSigner(privateKeyPem: string): Signer {
  const privateKey = createPrivateKey(privateKeyPem);
  if (privateKey.asymmetricKeyType !== "ed25519") {
    throw new Error("the licence private key must be an Ed25519 key");
  }
  const publicKey = createPublicKey(privateKey);
  return {
    sign: (payload) => signWith(privateKey, payload),
    verify: (signed) => verifyWith(publicKey, signed),
    // The last 32 bytes of the DER form are the raw public key.
    publicKey: publicKey.export({ format: "der", type: "spki" }).subarray(-32).toString("base64"),
  };
}

function signWith(privateKey: KeyObject, payload: string): string {
  const bytes = Buffer.from(payload, "utf8");
  const signature = sign(null, bytes, privateKey);
  return `${bytes.toString("base64url")}.${signature.toString("base64url")}`;
}

/** Returns the payload if we signed it, otherwise nothing. */
function verifyWith(publicKey: KeyObject, signed: string): string | null {
  const parts = signed.split(".");
  if (parts.length !== 2) return null;
  const [payload, signature] = parts.map((part) => Buffer.from(part, "base64url"));
  if (!payload || !signature || signature.length !== 64) return null;
  if (!verify(null, payload, publicKey, signature)) return null;
  return payload.toString("utf8");
}

/** Keys get pasted from emails: spaces and line breaks are forgiven, as in the app. */
export function tidyKey(key: string): string {
  return key.replace(/\s+/g, "");
}

/**
 * How a licence is named everywhere outside the key itself: in the store, in
 * receipts, and in the list of refunded keys. The app computes the same value.
 */
export function keyId(key: string): string {
  return createHash("sha256").update(tidyKey(key), "utf8").digest("hex");
}

export function issueKey(signer: Signer, licence: Licence): string {
  const { email, plan, issued } = licenceSchema.parse(licence);
  return signer.sign(JSON.stringify({ email, plan, issued }));
}

/** Returns what a key says if it is one of ours. */
export function readKey(signer: Signer, key: string): Licence | null {
  const payload = signer.verify(tidyKey(key));
  if (payload === null) return null;
  try {
    return licenceSchema.parse(JSON.parse(payload));
  } catch {
    return null;
  }
}

export function issueReceipt(signer: Signer, receipt: Receipt): string {
  return signer.sign(JSON.stringify({ licence: receipt.licence, device: receipt.device }));
}
