/**
 * The decisions behind automatic catching, kept free of browser calls so they
 * can be tested. This file looks only at ordinary browser downloads: it knows
 * nothing about pages, media or particular sites.
 */

/** Smaller files stay in the browser. Becomes a setting with the rules page (M2). */
export const MIN_CATCH_BYTES = 1024 * 1024;

/** Longest `Cookie` header passed on; anything longer is left out. */
const MAX_COOKIE_HEADER = 32 * 1024;

/** The parts of a browser download item the decision needs. */
export interface DownloadLike {
  url: string;
  /** Chrome only: where the download ended up after redirects. */
  finalUrl?: string;
  state: string;
  /** Unknown sizes are reported as -1 or 0. */
  totalBytes: number;
  filename?: string;
  referrer?: string;
  /** Chrome only: set when another extension started the download. */
  byExtensionId?: string;
}

export function downloadUrl(item: DownloadLike): string {
  return item.finalUrl || item.url;
}

/** Whether a new browser download should be handed to the app. */
export function shouldCatch(item: DownloadLike, autoCatch: boolean): boolean {
  if (!autoCatch) return false;
  // Old or finished entries are history, not something to take over.
  if (item.state !== "in_progress") return false;
  // Never interfere with what another extension is doing.
  if (item.byExtensionId) return false;
  // Pages can save data they made themselves (blob:, data:); only real links can move.
  if (!/^https?:\/\//i.test(downloadUrl(item))) return false;
  // A known small file is quicker left alone. An unknown size is caught.
  if (item.totalBytes > 0 && item.totalBytes < MIN_CATCH_BYTES) return false;
  return true;
}

/** The last part of the path the browser chose, if it has chosen one yet. */
export function suggestedName(item: DownloadLike): string | undefined {
  const name = item.filename?.split(/[/\\]/).pop()?.trim();
  return name ? name.slice(0, 255) : undefined;
}

/** Joins cookies into a `Cookie` header value, or nothing if there are none. */
export function cookieHeader(cookies: { name: string; value: string }[]): string | undefined {
  const header = cookies.map((cookie) => `${cookie.name}=${cookie.value}`).join("; ");
  return header && header.length <= MAX_COOKIE_HEADER ? header : undefined;
}
