import { z } from "zod";

/**
 * Messages between the extension, the native messaging host and the app.
 * The Rust side of this contract is apps/desktop/src-tauri/src/ipc.rs, plus one
 * fixed reply in crates/nm-host/src/main.rs.
 */
export const PROTOCOL_VERSION = 1;

/** A header value: one line, no control characters, bounded. */
const headerValue = z
  .string()
  .min(1)
  .max(32 * 1024)
  // eslint-disable-next-line no-control-regex
  .regex(/^[^\u0000-\u001f\u007f]*$/);

/**
 * Extension to app: download this ordinary file link. The optional fields are
 * what the browser would have sent, so a download behind a login still works.
 * The app keeps cookies in memory only and never logs them.
 */
export const downloadRequestSchema = z.object({
  type: z.literal("download"),
  version: z.literal(PROTOCOL_VERSION),
  url: z.url({ protocol: /^https?$/ }),
  filename: z.string().min(1).max(255).optional(),
  referrer: headerValue.optional(),
  userAgent: headerValue.optional(),
  /** The value of the `Cookie` header. */
  cookies: headerValue.optional(),
});
export type DownloadRequest = z.infer<typeof downloadRequestSchema>;

export const refusalCodeSchema = z.enum([
  "app_not_running",
  "invalid_message",
  "unsupported_version",
  "unsupported_url",
  "engine_unavailable",
  "rejected",
]);
export type RefusalCode = z.infer<typeof refusalCodeSchema>;

/** App to extension: the only thing that flows back. */
export const ackSchema = z.discriminatedUnion("ok", [
  z.object({ type: z.literal("ack"), ok: z.literal(true), id: z.string().min(1) }),
  z.object({ type: z.literal("ack"), ok: z.literal(false), code: refusalCodeSchema }),
]);
export type Ack = z.infer<typeof ackSchema>;
