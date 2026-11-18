import {
  NATIVE_HOST_NAME,
  PROTOCOL_VERSION,
  ackSchema,
  downloadRequestSchema,
} from "@mbiro/protocol";
import { browser } from "wxt/browser";
import { defineBackground } from "wxt/utils/define-background";
import { cookieHeader, downloadUrl, shouldCatch, suggestedName } from "../capture";
import { strings } from "../strings";

const MENU_ID = "download-link";
const AUTO_CATCH_KEY = "autoCatch";

interface Outcome {
  ok: boolean;
  message: string;
}

async function autoCatchEnabled(): Promise<boolean> {
  const stored = await browser.storage.local.get(AUTO_CATCH_KEY);
  return stored[AUTO_CATCH_KEY] !== false; // on unless switched off
}

/** The cookies the browser itself would send to this address. Never stored or logged here. */
async function cookiesFor(url: string): Promise<string | undefined> {
  try {
    return cookieHeader(await browser.cookies.getAll({ url }));
  } catch {
    return undefined;
  }
}

/** Sends one link to the app and returns the sentence that describes what happened. */
async function sendToApp(link: {
  url: string;
  filename?: string;
  referrer?: string;
}): Promise<Outcome> {
  const request = downloadRequestSchema.safeParse({
    type: "download",
    version: PROTOCOL_VERSION,
    url: link.url,
    filename: link.filename || undefined,
    referrer: link.referrer || undefined,
    userAgent: navigator.userAgent,
    cookies: await cookiesFor(link.url),
  });
  if (!request.success) return { ok: false, message: strings.refused.unsupported_url };

  let reply: unknown;
  try {
    reply = await browser.runtime.sendNativeMessage(NATIVE_HOST_NAME, request.data);
  } catch {
    // The browser could not start the host: the app has never run on this Mac.
    return { ok: false, message: strings.noAnswer };
  }
  const ack = ackSchema.safeParse(reply);
  if (!ack.success) return { ok: false, message: strings.noAnswer };
  return ack.data.ok
    ? { ok: true, message: strings.sent }
    : { ok: false, message: strings.refused[ack.data.code] };
}

/** Toolbar feedback for now: the designed popup and in-page notice come in M2. */
async function show(outcome: Outcome | "off" | "on"): Promise<void> {
  if (outcome === "off" || outcome === "on") {
    await browser.action.setBadgeText({ text: outcome === "off" ? strings.badgeOff : "" });
    await browser.action.setTitle({
      title: outcome === "off" ? strings.catchingOff : strings.catchingOn,
    });
    return;
  }
  await browser.action.setBadgeText({ text: outcome.ok ? strings.badgeOk : strings.badgeFailed });
  await browser.action.setTitle({ title: outcome.message });
}

/**
 * Takes a new browser download over. The browser's copy is paused while the app
 * is asked, removed if the app accepts, and resumed untouched if it does not,
 * so a download is never lost.
 */
async function catchDownload(item: Browser.downloads.DownloadItem): Promise<void> {
  if (!shouldCatch(item, await autoCatchEnabled())) return;
  try {
    await browser.downloads.pause(item.id);
  } catch {
    return; // already finished or gone: leave it with the browser
  }

  const outcome = await sendToApp({
    url: downloadUrl(item),
    filename: suggestedName(item),
    referrer: item.referrer,
  });

  try {
    if (outcome.ok) {
      await browser.downloads.cancel(item.id);
      await browser.downloads.erase({ id: item.id });
    } else {
      await browser.downloads.resume(item.id);
    }
  } catch {
    // The user or the browser got there first; nothing more to do.
  }
  await show(outcome);
}

export default defineBackground(() => {
  browser.runtime.onInstalled.addListener(() => {
    browser.contextMenus.create({
      id: MENU_ID,
      title: strings.menuDownload,
      contexts: ["link"],
    });
  });

  browser.contextMenus.onClicked.addListener((info) => {
    if (info.menuItemId !== MENU_ID || !info.linkUrl) return;
    void sendToApp({ url: info.linkUrl, referrer: info.pageUrl }).then(show);
  });

  browser.downloads.onCreated.addListener((item) => void catchDownload(item));

  // Until the popup exists, a click on the toolbar button switches catching on or off.
  browser.action.onClicked.addListener(() => {
    void autoCatchEnabled().then(async (enabled) => {
      await browser.storage.local.set({ [AUTO_CATCH_KEY]: !enabled });
      await show(enabled ? "off" : "on");
    });
  });
});
