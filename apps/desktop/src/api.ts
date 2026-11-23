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
  /** True while a file is coming down in a way that probably cannot be resumed. */
  resumeDoubtful: boolean;
  /** The category its name falls in, if any. */
  category: string | null;
}

/** Mirrors `EngineHealth`, `ToolStatus` and `ToolProgress` in src-tauri/src/lib.rs. */
export type EngineHealth =
  { status: "ready"; version: string } | { status: "failed"; reason: string };

export interface ToolStatus {
  name: string;
  version: string;
  installed: boolean;
  /** Only yt-dlp is kept up to date on its own. */
  updatable: boolean;
  /** When updates were last looked for, in seconds since 1970. */
  checkedAt: number | null;
}

export interface ToolProgress {
  name: string;
  received: number;
  total: number | null;
}

/** Mirrors `Quality`, `Preset`, `FormatRow` and `VideoInfo` in src-tauri/src/video.rs. */
export type Quality =
  | { kind: "best" }
  | { kind: "max"; height: number }
  | { kind: "audio" }
  | { kind: "format"; id: string; audio_only: boolean };

export interface Preset {
  quality: Quality;
  height: number | null;
  ext: string;
  approxBytes: number | null;
}

export interface FormatRow {
  quality: Quality;
  height: number | null;
  fps: number | null;
  ext: string;
  hasVideo: boolean;
  hasAudio: boolean;
  approxBytes: number | null;
}

export interface VideoInfo {
  title: string;
  durationSecs: number | null;
  host: string;
  /** A `data:` URL, or null when the page has no picture. */
  thumbnail: string | null;
  presets: Preset[];
  formats: FormatRow[];
}

export interface PlaylistEntry {
  url: string;
  title: string;
  durationSecs: number | null;
}

export interface PlaylistInfo {
  title: string;
  host: string;
  entries: PlaylistEntry[];
}

/** What a link turned out to be. Mirrors `Found` in src-tauri/src/video.rs. */
export type Found = ({ kind: "video" } & VideoInfo) | ({ kind: "playlist" } & PlaylistInfo);

/** Mirrors `Settings` and `Category` in src-tauri/src/settings.rs. */
export interface Category {
  name: string;
  extensions: string[];
  /** Null saves into the default folder. */
  dir: string | null;
}

export interface Settings {
  /** Null is the Mac's Downloads folder. */
  defaultDir: string | null;
  maxConcurrent: number;
  defaultConnections: number;
  /** Null is no cap. */
  speedLimitKbps: number | null;
  watchClipboard: boolean;
  notifyOnFinish: boolean;
  /** Follow yt-dlp's nightly builds instead of its stable releases. */
  ytDlpNightly: boolean;
  /** The first-launch steps have been gone through. */
  onboarded: boolean;
  categories: Category[];
}

/** Mirrors `BrowserStatus` in src-tauri/src/browsers.rs. */
export interface BrowserStatus {
  name: string;
  installed: boolean;
  connected: boolean;
}

/** Mirrors `Status` in src-tauri/src/licence.rs. */
export type LicenceStatus =
  | { state: "licensed"; email: string; plan: string }
  | { state: "trial"; daysLeft: number }
  | { state: "expired" };

/** Mirrors `UpdateStatus` in src-tauri/src/lib.rs. */
export interface UpdateStatus {
  /** False until the build names an update key and a manifest. */
  configured: boolean;
  current: string;
  available: { version: string; notes: string | null } | null;
  checkedAt: number | null;
}

export interface UpdateProgress {
  received: number;
  total: number | null;
}

/** Mirrors `Acknowledgement` in src-tauri/src/acknowledgements.rs. */
export interface Acknowledgement {
  name: string;
  version: string | null;
  licence: string;
  role: string;
  /** True if it is inside the app; false if the app fetches it on request. */
  shipped: boolean;
  source: string;
  licenceLink: string | null;
  text: string | null;
}

export interface AddDefaults {
  dir: string;
  connections: number;
  maxConnections: number;
}

export const SNAPSHOT_EVENT = "downloads:snapshot";
export const TOOL_PROGRESS_EVENT = "tools:progress";
export const COPIED_LINK_EVENT = "clipboard:link";
export const OPEN_ADD_EVENT = "add:open";
export const UPDATE_EVENT = "update:status";
export const UPDATE_PROGRESS_EVENT = "update:progress";

/** Every command the window may send to the core. */
export const api = {
  engineHealth: () => invoke<EngineHealth>("engine_health"),
  list: () => invoke<Job[]>("list_downloads"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  browserStatus: () => invoke<BrowserStatus[]>("browser_status"),
  licenceStatus: () => invoke<LicenceStatus>("licence_status"),
  enterLicence: (key: string) => invoke<LicenceStatus>("enter_licence", { key }),
  removeLicence: () => invoke<LicenceStatus>("remove_licence"),
  acknowledgements: () => invoke<Acknowledgement[]>("acknowledgements"),
  updateStatus: () => invoke<UpdateStatus>("update_status"),
  checkForUpdate: () => invoke<UpdateStatus>("check_for_update"),
  installUpdate: () => invoke<null>("install_update"),
  addDefaults: () => invoke<AddDefaults>("add_defaults"),
  add: (url: string, dir: string | null, connections: number) =>
    invoke<number>("add_download", { url, dir, connections }),
  probeVideo: (url: string) => invoke<Found>("probe_video", { url }),
  addPlaylist: (items: { url: string; title: string }[], dir: string | null, quality: Quality) =>
    invoke<number>("add_playlist", { items, dir, quality }),
  addVideo: (url: string, dir: string | null, quality: Quality | null, title: string | null) =>
    invoke<number>("add_video", { url, dir, quality, title }),
  pause: (id: number) => invoke<null>("pause_download", { id }),
  resume: (id: number) => invoke<null>("resume_download", { id }),
  retry: (id: number) => invoke<null>("retry_download", { id }),
  remove: (id: number) => invoke<null>("remove_download", { id }),
  pauseAll: () => invoke<null>("pause_all"),
  resumeAll: () => invoke<null>("resume_all"),
  showFile: (id: number) => invoke<null>("show_file", { id }),
  openMainWindow: () => invoke<null>("open_main_window"),
  copiedLink: () => invoke<string | null>("copied_link"),
  useCopiedLink: () => invoke<null>("use_copied_link"),
  toolsStatus: () => invoke<ToolStatus[]>("tools_status"),
  updateTools: () => invoke<null>("update_tools"),
  fetchTools: () => invoke<null>("fetch_tools"),
};
