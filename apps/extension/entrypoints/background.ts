import {
  NATIVE_HOST_NAME,
  PROTOCOL_VERSION,
  ackSchema,
  downloadRequestSchema,
  type DownloadRequest,
} from "@mbiro/protocol";
import { browser } from "wxt/browser";
import { defineBackground } from "wxt/utils/define-background";
import { strings } from "../strings";

const MENU_ID = "download-link";

/** Sends one link to the app and returns the sentence that describes what happened. */
async function sendToApp(url: string): Promise<{ ok: boolean; message: string }> {
  const request = downloadRequestSchema.safeParse({
    type: "download",
    version: PROTOCOL_VERSION,
    url,
  } satisfies DownloadRequest);
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
    void sendToApp(info.linkUrl).then(async ({ ok, message }) => {
      // Spike feedback only: the designed popup and in-page notice come in M2.
      await browser.action.setBadgeText({ text: ok ? strings.badgeOk : strings.badgeFailed });
      await browser.action.setTitle({ title: message });
    });
  });
});
