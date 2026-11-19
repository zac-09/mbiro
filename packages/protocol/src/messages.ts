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

/** Extension to app: how are these downloads, which I sent earlier, getting on? */
export const statusRequestSchema = z.object({
  type: z.literal("status"),
  version: z.literal(PROTOCOL_VERSION),
  ids: z.array(z.string().min(1)).max(50),
});
export type StatusRequest = z.infer<typeof statusRequestSchema>;

/**
 * Extension to app: bring the window forward. The native messaging host starts
 * the app first if it is closed, which it recognises by this exact `type`.
 */
export const openRequestSchema = z.object({
  type: z.literal("open"),
  version: z.literal(PROTOCOL_VERSION),
});
export type OpenRequest = z.infer<typeof openRequestSchema>;

/** Extension to app: the browser will download this after all; drop it. */
export const cancelRequestSchema = z.object({
  type: z.literal("cancel"),
  version: z.literal(PROTOCOL_VERSION),
  id: z.string().min(1),
});
export type CancelRequest = z.infer<typeof cancelRequestSchema>;

export const jobStateSchema = z.enum(["queued", "active", "paused", "done", "failed"]);

/** One download the extension asked about. */
export const jobSummarySchema = z.object({
  id: z.string(),
  name: z.string(),
  state: jobStateSchema,
  totalBytes: z.number().nonnegative(),
  completedBytes: z.number().nonnegative(),
  /** Seconds since 1970. */
  finishedAt: z.number().nullable(),
});
export type JobSummary = z.infer<typeof jobSummarySchema>;

/** App to extension: the answer to a status request. */
export const statusReplySchema = z.object({
  type: z.literal("status"),
  jobs: z.array(jobSummarySchema),
});
export type StatusReply = z.infer<typeof statusReplySchema>;

/** App to extension: yes or no. `id` names the download a download request started. */
export const ackSchema = z.discriminatedUnion("ok", [
  z.object({ type: z.literal("ack"), ok: z.literal(true), id: z.string().min(1).optional() }),
  z.object({ type: z.literal("ack"), ok: z.literal(false), code: refusalCodeSchema }),
]);
export type Ack = z.infer<typeof ackSchema>;
