import { PRODUCT_NAME, type JobSummary } from "@mbiro/protocol";
import { clock, formatSize, percent } from "./format";

const NAME = PRODUCT_NAME.toUpperCase();

/**
 * Every user-facing string of the extension, in one place for translation later.
 * Labels are written in the case they are shown in.
 */
export const strings = {
  wordmark: NAME,
  menuDownload: `Download with ${PRODUCT_NAME}`,

  popup: {
    connected: "APP CONNECTED",
    notFound: "APP NOT FOUND",
    catchDownloads: "CATCH DOWNLOADS",
    allSites: "ALL SITES",
    thisSite: "THIS SITE",
    on: "ON",
    off: "OFF",
    sentToday: `SENT TO ${NAME} TODAY`,
    nothingSent: "NOTHING YET",
    settings: "SETTINGS",
    openApp: `OPEN ${NAME}`,
    offlineHeading: "THE APP ISN’T RUNNING",
    offlineBody: `Downloads will stay in the browser until ${PRODUCT_NAME} is open on this Mac. Nothing is lost in the meantime.`,
    getApp: "GET THE APP",
    tryAgain: "TRY AGAIN",
    job: (job: JobSummary): string => {
      const size = job.totalBytes > 0 ? formatSize(job.totalBytes) : null;
      const pct = `${percent(job.completedBytes, job.totalBytes)}%`;
      const parts: Record<JobSummary["state"], (string | null)[]> = {
        active: [size, pct, "DOWNLOADING"],
        queued: [size, "QUEUED"],
        paused: [size, pct, "PAUSED"],
        done: [size, job.finishedAt ? `DONE ${clock(job.finishedAt)}` : "DONE"],
        failed: [size, "FAILED"],
      };
      return parts[job.state].filter(Boolean).join(" · ");
    },
  },

  toast: {
    sent: `SENT TO ${NAME}`,
    useBrowser: "USE BROWSER INSTEAD",
    show: `SHOW IN ${NAME}`,
  },

  rules: {
    title: `${PRODUCT_NAME}: capture rules`,
    heading: "CAPTURE RULES",
    product: `${NAME} BROWSER EXTENSION`,
    auto: "CATCH DOWNLOADS AUTOMATICALLY",
    autoHelp: "When off, use the right-click menu on a link.",
    larger: "ONLY FILES LARGER THAN",
    largerHelp: "Smaller files stay in the browser.",
    mb: "MB",
    types: "FILE TYPES TO CATCH",
    editList: "EDIT LIST",
    done: "DONE",
    typesLabel: "File types, separated by commas",
    sites: "NEVER CATCH ON THESE SITES",
    addSite: "ADD SITE",
    add: "ADD",
    sitePlaceholder: "example.com",
    siteLabel: "Site to leave alone",
    siteInvalid: "That is not a site name. Enter something like example.com.",
    noSites: "No sites yet.",
    remove: "REMOVE",
  },
} as const;
