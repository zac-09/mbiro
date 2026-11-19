import {
  NATIVE_HOST_NAME,
  PROTOCOL_VERSION,
  ackSchema,
  downloadRequestSchema,
  statusReplySchema,
  type Ack,
  type JobSummary,
} from "@mbiro/protocol";
import { browser } from "wxt/browser";

/** Sends one message to the app through the native messaging host. Null means no usable answer. */
async function ask(message: object): Promise<unknown> {
  try {
    return await browser.runtime.sendNativeMessage(NATIVE_HOST_NAME, message);
  } catch {
    // The browser could not start the host: the app has never run on this Mac.
    return null;
  }
}

async function askForAck(message: object): Promise<Ack | null> {
  const ack = ackSchema.safeParse(await ask(message));
  return ack.success ? ack.data : null;
}

export interface LinkToSend {
  url: string;
  filename?: string;
  referrer?: string;
  cookies?: string;
}

/** Hands a link to the app. Null means the link was not one the app could take. */
export async function sendDownload(link: LinkToSend): Promise<Ack | null> {
  const request = downloadRequestSchema.safeParse({
    type: "download",
    version: PROTOCOL_VERSION,
    url: link.url,
    filename: link.filename || undefined,
    referrer: link.referrer || undefined,
    userAgent: navigator.userAgent,
    cookies: link.cookies,
  });
  return request.success ? askForAck(request.data) : null;
}

/** How the downloads with these ids are getting on. Null means the app is not reachable. */
export async function fetchStatus(ids: string[]): Promise<JobSummary[] | null> {
  const reply = statusReplySchema.safeParse(
    await ask({ type: "status", version: PROTOCOL_VERSION, ids: ids.slice(0, 50) }),
  );
  return reply.success ? reply.data.jobs : null;
}

/** Brings the app forward, starting it first if it is closed. True if it answered. */
export async function openApp(): Promise<boolean> {
  return (await askForAck({ type: "open", version: PROTOCOL_VERSION }))?.ok === true;
}

/** Takes a download back from the app. */
export async function cancelDownload(id: string): Promise<boolean> {
  return (await askForAck({ type: "cancel", version: PROTOCOL_VERSION, id }))?.ok === true;
}
