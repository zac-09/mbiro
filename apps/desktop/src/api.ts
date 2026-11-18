import { invoke } from "@tauri-apps/api/core";

/** Mirrors `JobView` in src-tauri/src/jobs.rs. */
export type JobState = "queued" | "active" | "paused" | "done" | "failed";
export type JobKind = "file" | "video";

export interface Job {
  id: number;
  kind: JobKind;
  state: JobState;
  name: string;
  dir: string;
  totalBytes: number;
  completedBytes: number;
  speedBps: number;
  connections: number;
  error: string | null;
  /** Seconds since 1970. */
  createdAt: number;
  finishedAt: number | null;
}

/** Mirrors `EngineHealth`, `ToolStatus` and `ToolProgress` in src-tauri/src/lib.rs. */
export type EngineHealth =
  { status: "ready"; version: string } | { status: "failed"; reason: string };

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

export interface AddDefaults {
  dir: string;
  connections: number;
  maxConnections: number;
}

export const SNAPSHOT_EVENT = "downloads:snapshot";
export const TOOL_PROGRESS_EVENT = "tools:progress";

/** Every command the window may send to the core. */
export const api = {
  engineHealth: () => invoke<EngineHealth>("engine_health"),
  list: () => invoke<Job[]>("list_downloads"),
  addDefaults: () => invoke<AddDefaults>("add_defaults"),
  add: (url: string, dir: string | null, connections: number) =>
    invoke<number>("add_download", { url, dir, connections }),
  addVideo: (url: string, dir: string | null) => invoke<number>("add_video", { url, dir }),
  pause: (id: number) => invoke<null>("pause_download", { id }),
  resume: (id: number) => invoke<null>("resume_download", { id }),
  retry: (id: number) => invoke<null>("retry_download", { id }),
  remove: (id: number) => invoke<null>("remove_download", { id }),
  pauseAll: () => invoke<null>("pause_all"),
  resumeAll: () => invoke<null>("resume_all"),
  showFile: (id: number) => invoke<null>("show_file", { id }),
  toolsStatus: () => invoke<ToolStatus[]>("tools_status"),
  fetchTools: () => invoke<null>("fetch_tools"),
};
