/**
 * The decisions behind automatic catching, kept free of browser calls so they
 * can be tested. This file looks only at ordinary browser downloads: it knows
 * nothing about pages, media or particular sites.
 */

/** The capture rules the user can change. */
export interface Settings {
  autoCatch: boolean;
  /** Files known to be smaller than this stay in the browser. */
  minSizeMb: number;
  /** Extensions, lower case, without the dot. */
  fileTypes: string[];
  /** Host names never to catch on. A name also covers its subdomains. */
  excludedSites: string[];
}

/** The defaults shown in the designed rules page. */
export const DEFAULT_SETTINGS: Settings = {
  autoCatch: true,
  minSizeMb: 5,
  fileTypes: ["zip", "dmg", "pkg", "iso", "pdf", "mp4", "mkv", "mp3", "tar.gz", "7z", "exe"],
  excludedSites: [],
};

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

/** The host of an http or https address, lower case and without `www.`. */
export function siteOf(address: string | undefined): string | undefined {
  if (!address) return undefined;
  try {
    const url = new URL(address);
    if (url.protocol !== "http:" && url.protocol !== "https:") return undefined;
    return url.hostname.toLowerCase().replace(/^www\./, "") || undefined;
  } catch {
    return undefined;
  }
}

/** Whether a host is on the list, itself or as a subdomain of a listed name. */
export function isExcluded(host: string | undefined, sites: string[]): boolean {
  if (!host) return false;
  return sites.some((site) => host === site || host.endsWith(`.${site}`));
}

/** A host name as the user would type it into the list, tidied; or nothing if it is not one. */
export function cleanSite(text: string): string | undefined {
  const trimmed = text.trim().toLowerCase();
  const host = siteOf(trimmed.includes("://") ? trimmed : `https://${trimmed}`);
  return host && /^[a-z0-9.-]+$/.test(host) && host.includes(".") ? host : undefined;
}

/** A file type as the user would type it, tidied; or nothing if it is not one. */
export function cleanType(text: string): string | undefined {
  const type = text.trim().toLowerCase().replace(/^\.+/, "");
  return /^[a-z0-9]+(\.[a-z0-9]+)?$/.test(type) && type.length <= 12 ? type : undefined;
}

/** The last part of the path the browser chose, if it has chosen one yet. */
export function suggestedName(item: DownloadLike): string | undefined {
  const name = item.filename?.split(/[/\\]/).pop()?.trim();
  return name ? name.slice(0, 255) : undefined;
}

/** The name to judge a download by: the browser's choice, else the end of the address. */
export function displayName(item: DownloadLike): string {
  const chosen = suggestedName(item);
  if (chosen) return chosen;
  try {
    const last = new URL(downloadUrl(item)).pathname.split("/").pop() ?? "";
    return decodeURIComponent(last) || new URL(downloadUrl(item)).hostname;
  } catch {
    return "";
  }
}

/** The extensions a name ends in, longest first: `a.tar.gz` gives `tar.gz` and `gz`. */
export function extensionsOf(name: string): string[] {
  const parts = name.toLowerCase().split(".").slice(1);
  if (parts.length === 0 || parts.some((part) => !/^[a-z0-9]{1,8}$/.test(part))) {
    const last = /\.([a-z0-9]{1,8})$/i.exec(name)?.[1]?.toLowerCase();
    return last ? [last] : [];
  }
  const last = parts[parts.length - 1] as string;
  return parts.length >= 2 ? [`${parts[parts.length - 2]}.${last}`, last] : [last];
}

/** Whether a new browser download should be handed to the app. */
export function shouldCatch(item: DownloadLike, settings: Settings): boolean {
  if (!settings.autoCatch) return false;
  // Old or finished entries are history, not something to take over.
  if (item.state !== "in_progress") return false;
  // Never interfere with what another extension is doing.
  if (item.byExtensionId) return false;
  // Pages can save data they made themselves (blob:, data:); only real links can move.
  if (!/^https?:\/\//i.test(downloadUrl(item))) return false;
  // The site is the page the download started from, or failing that its own host.
  const site = siteOf(item.referrer) ?? siteOf(downloadUrl(item));
  if (isExcluded(site, settings.excludedSites)) return false;
  // A known small file is quicker left alone. An unknown size is caught.
  if (item.totalBytes > 0 && item.totalBytes < settings.minSizeMb * 1024 * 1024) return false;
  // A recognisable type that is not on the list stays; a name with no type is caught.
  const types = extensionsOf(displayName(item));
  if (types.length > 0 && !types.some((type) => settings.fileTypes.includes(type))) return false;
  return true;
}

/** Joins cookies into a `Cookie` header value, or nothing if there are none. */
export function cookieHeader(cookies: { name: string; value: string }[]): string | undefined {
  const header = cookies.map((cookie) => `${cookie.name}=${cookie.value}`).join("; ");
  return header && header.length <= MAX_COOKIE_HEADER ? header : undefined;
}
