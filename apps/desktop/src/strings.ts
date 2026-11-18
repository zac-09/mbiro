import { PRODUCT_NAME } from "@mbiro/protocol";

/** Every user-facing string of the desktop UI, in one place for translation later. */
export const strings = {
  title: `${PRODUCT_NAME} · engine spike`,
  urlLabel: "File URL",
  urlPlaceholder: "https://example.com/file.zip",
  add: "Add",
  addVideo: "Add as video",
  urlLabelHint: "A link to a file, or to a page with a video on it.",
  toolsHeading: "Video tools",
  toolsIntro:
    "Video downloads need three helper tools. They are downloaded once, checked, and kept on this Mac.",
  toolsFetch: "Fetch tools",
  toolsFetching: "Fetching…",
  toolInstalled: "installed",
  toolMissing: "not installed",
  toolProgress: (percent: number) => `${percent}%`,
  pause: "Pause",
  resume: "Resume",
  remove: "Remove",
  empty: "No downloads yet. Paste a link to a file and press Add.",
  totalSpeed: "Total speed",
  connections: (n: number) => `${n} conn`,
  engineStarting: "Starting the download engine…",
  engineFailed: "The download engine did not start.",
  state: {
    queued: "Queued",
    active: "Downloading",
    paused: "Paused",
    done: "Done",
    failed: "Failed",
  },
} as const;
