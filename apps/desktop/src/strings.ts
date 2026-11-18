import { PRODUCT_NAME } from "@mbiro/protocol";

/**
 * Every user-facing string of the desktop UI, in one place for translation later.
 * Labels are written in the case they are shown in.
 */
export const strings = {
  wordmark: PRODUCT_NAME.toUpperCase(),
  tabs: { all: "ALL", active: "ACTIVE", done: "DONE", failed: "FAILED" },
  search: "SEARCH",
  searchLabel: "Search downloads",
  speedUnit: "MB/S",
  counts: (active: number, paused: number, failed: number) =>
    `${active} ACTIVE · ${paused} PAUSED · ${failed} FAILED`,
  pauseAll: "PAUSE ALL",
  resumeAll: "RESUME ALL",
  addDownload: "+ ADD DOWNLOAD",
  listLabel: "Downloads",
  empty: "No downloads yet. Press Add download, or paste a link.",
  emptyFiltered: "Nothing here.",
  engineFailed: "The download engine did not start. Restart the app.",

  row: {
    pause: "PAUSE",
    resume: "RESUME",
    showFile: "SHOW FILE",
    retry: "RETRY",
    remove: "REMOVE",
    confirmRemove: "REMOVE?",
    keep: "KEEP",
    paused: "PAUSED",
    queued: "QUEUED",
    failed: "FAILED",
    done: (time: string) => `DONE · ${time}`,
    speed: (mb: string, left: string | null) => (left ? `${mb} MB/S · ${left}` : `${mb} MB/S`),
    progress: (done: string, total: string) => `${done} / ${total}`,
    connections: (n: number) => `${n} CONN`,
    video: "VIDEO",
    stateLabel: {
      queued: "queued",
      active: "downloading",
      paused: "paused",
      done: "done",
      failed: "failed",
    },
  },

  menuBar: {
    openWindow: "OPEN WINDOW",
    summary: (active: number, paused: number, failed: number, more: number) => {
      const parts = [
        more > 0 ? `${more} MORE` : null,
        paused > 0 ? `${paused} PAUSED` : null,
        failed > 0 ? `${failed} FAILED` : null,
      ].filter(Boolean);
      if (parts.length > 0) return parts.join(" · ");
      return active > 0 ? "" : "NOTHING DOWNLOADING";
    },
  },

  add: {
    heading: "ADD DOWNLOAD",
    link: "LINK",
    linkPlaceholder: "https://",
    type: "TYPE",
    file: "FILE",
    video: "VIDEO PAGE",
    connections: "CONNECTIONS",
    saveTo: "SAVE TO",
    change: "CHANGE",
    cancel: "CANCEL",
    download: "DOWNLOAD",
    reading: "READING THE PAGE…",
    thumbnail: "THUMBNAIL",
    videoMeta: (length: string | null, host: string) =>
      ["VIDEO", length, host.toUpperCase()].filter(Boolean).join(" · "),
    quality: "QUALITY",
    best: "BEST AVAILABLE",
    audioOnly: "AUDIO ONLY",
    height: (pixels: number) => `${pixels}P`,
    approx: (size: string) => `~${size}`,
    fps: (frames: number) => `${frames} FPS`,
    noSound: "NO SOUND",
    showAll: "SHOW ALL FORMATS",
    showFewer: "SHOW FEWER FORMATS",
    toolsNeeded:
      "Video downloads need three helper tools. They are downloaded once, checked, and kept on this Mac.",
    toolsFetch: "FETCH TOOLS",
    toolsFetching: (percent: number | null, name: string) =>
      percent === null
        ? `FETCHING ${name.toUpperCase()}`
        : `FETCHING ${name.toUpperCase()} · ${percent}%`,
  },
} as const;
