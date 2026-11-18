/** Mirrors `JobStatus` in src-tauri/src/engine/mod.rs. */
export type JobState = "queued" | "active" | "paused" | "done" | "failed";

export interface JobStatus {
  id: string;
  state: JobState;
  name: string;
  totalBytes: number;
  completedBytes: number;
  speedBps: number;
  connections: number;
  error: string | null;
}

/** Mirrors `EngineHealth` in src-tauri/src/lib.rs. */
export type EngineHealth =
  | { status: "starting" }
  | { status: "ready"; version: string }
  | { status: "failed"; reason: string };

export const SNAPSHOT_EVENT = "downloads:snapshot";

/** Mirrors `ToolStatus` and `ToolProgress` in src-tauri/src/lib.rs. */
export interface ToolStatus {
  name: string;
  version: string;
  installed: boolean;
}

export interface ToolProgress {
  name: string;
  received: number;
  total: number | null;
}

export const TOOL_PROGRESS_EVENT = "tools:progress";
