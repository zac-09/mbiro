import { browser } from "wxt/browser";

const KEY = "sent";
const KEEP = 20;

/** One download handed to the app. Only what the popup needs to ask about it later. */
export interface Sent {
  id: string;
  /** Milliseconds since 1970. */
  at: number;
}

/** Entries from today, newest first. */
export function todays(entries: Sent[], now: Date): Sent[] {
  const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  return entries.filter((entry) => entry.at >= midnight).sort((a, b) => b.at - a.at);
}

export async function loadSentToday(): Promise<Sent[]> {
  const stored: unknown = (await browser.storage.local.get(KEY))[KEY];
  const entries = Array.isArray(stored)
    ? stored.filter(
        (entry): entry is Sent =>
          typeof entry === "object" &&
          entry !== null &&
          typeof (entry as Sent).id === "string" &&
          typeof (entry as Sent).at === "number",
      )
    : [];
  return todays(entries, new Date());
}

export async function rememberSent(id: string): Promise<void> {
  const entries = [{ id, at: Date.now() }, ...(await loadSentToday())].slice(0, KEEP);
  await browser.storage.local.set({ [KEY]: entries });
}

export async function forgetSent(id: string): Promise<void> {
  const entries = (await loadSentToday()).filter((entry) => entry.id !== id);
  await browser.storage.local.set({ [KEY]: entries });
}
