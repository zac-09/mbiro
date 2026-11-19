import { browser } from "wxt/browser";
import { defineBackground } from "wxt/utils/define-background";
import { cancelDownload, openApp, sendDownload } from "../app";
import { cookieHeader, displayName, downloadUrl, shouldCatch, suggestedName } from "../capture";
import { formatSize } from "../format";
import { forgetSent, rememberSent } from "../sent";
import { loadSettings } from "../settings";
import { strings } from "../strings";
import { showToast, type ToastArgs, type ToastMessage } from "../toast";

const MENU_ID = "download-link";

/**
 * Addresses the user asked the browser to keep after all. The next download of
 * one is left alone, once. Lost if the browser unloads this worker, which only
 * means that download may be offered to the app again.
 */
const leaveAlone = new Set<string>();

/** The cookies the browser itself would send to this address. Never stored or logged here. */
async function cookiesFor(url: string): Promise<string | undefined> {
  try {
    return cookieHeader(await browser.cookies.getAll({ url }));
  } catch {
    return undefined;
  }
}

/** Shows the designed notice in the tab the user is looking at, where a page allows it. */
async function notify(args: ToastArgs): Promise<void> {
  try {
    const [tab] = await browser.tabs.query({ active: true, lastFocusedWindow: true });
    if (tab?.id === undefined) return;
    await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: showToast,
      args: [args],
    });
  } catch {
    // Browser pages and a few others cannot be drawn on. The download is in the app regardless.
  }
}

/** Hands one link to the app. Returns the app's id for it, or nothing if the app did not take it. */
async function send(link: {
  url: string;
  filename?: string;
  referrer?: string;
  name: string;
  size?: number;
}): Promise<string | undefined> {
  const ack = await sendDownload({ ...link, cookies: await cookiesFor(link.url) });
  if (!ack?.ok || !ack.id) return undefined;
  await rememberSent(ack.id);
  void notify({
    id: ack.id,
    url: link.url,
    heading: strings.toast.sent,
    detail: link.size && link.size > 0 ? formatSize(link.size) : "",
    name: link.name,
    useBrowser: strings.toast.useBrowser,
    show: strings.toast.show,
  });
  return ack.id;
}

/**
 * Takes a new browser download over. The browser's copy is paused while the app
 * is asked, removed if the app accepts, and resumed untouched if it does not,
 * so a download is never lost.
 */
async function catchDownload(item: Browser.downloads.DownloadItem): Promise<void> {
  const url = downloadUrl(item);
  if (leaveAlone.delete(url)) return;
  if (!shouldCatch(item, await loadSettings())) return;
  try {
    await browser.downloads.pause(item.id);
  } catch {
    return; // already finished or gone: leave it with the browser
  }

  const taken = await send({
    url,
    filename: suggestedName(item),
    referrer: item.referrer,
    name: displayName(item),
    size: item.totalBytes,
  });

  try {
    if (taken) {
      await browser.downloads.cancel(item.id);
      await browser.downloads.erase({ id: item.id });
    } else {
      await browser.downloads.resume(item.id);
    }
  } catch {
    // The user or the browser got there first; nothing more to do.
  }
}

/** A button on the in-page notice was pressed. */
async function onToast(message: ToastMessage): Promise<void> {
  if (message.kind === "toast-show") {
    await openApp();
    return;
  }
  // Give the download back to the browser: drop it in the app, then start it here.
  await cancelDownload(message.id);
  await forgetSent(message.id);
  leaveAlone.add(message.url);
  await browser.downloads.download({ url: message.url });
}

function isToastMessage(message: unknown): message is ToastMessage {
  if (typeof message !== "object" || message === null) return false;
  const { kind, id, url } = message as Record<string, unknown>;
  if (kind === "toast-show") return true;
  return (
    kind === "toast-use-browser" &&
    typeof id === "string" &&
    typeof url === "string" &&
    /^https?:\/\//i.test(url)
  );
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
    const url = info.linkUrl;
    void send({
      url,
      referrer: info.pageUrl,
      name: displayName({ url, state: "", totalBytes: 0 }),
    });
  });

  browser.downloads.onCreated.addListener((item) => void catchDownload(item));

  // Only this extension's own pages and its notice can reach this listener.
  browser.runtime.onMessage.addListener((message: unknown, sender) => {
    if (sender.id === browser.runtime.id && isToastMessage(message)) void onToast(message);
  });
});
