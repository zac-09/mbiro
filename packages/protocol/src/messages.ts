import { z } from "zod";

/**
 * Messages between the extension, the native messaging host and the app.
 * The Rust side of this contract is apps/desktop/src-tauri/src/ipc.rs, plus one
 * fixed reply in crates/nm-host/src/main.rs.
 */
export const PROTOCOL_VERSION = 1;

/** Extension to app: download this ordinary file link. */
export const downloadRequestSchema = z.object({
  type: z.literal("download"),
  version: z.literal(PROTOCOL_VERSION),
  url: z.url({ protocol: /^https?$/ }),
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
