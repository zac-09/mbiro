import { browser } from "wxt/browser";
import { DEFAULT_SETTINGS, cleanSite, cleanType, type Settings } from "./capture";

const KEY = "settings";

/** Whatever is stored, made safe to use: anything missing or odd falls back to the default. */
export function normalise(stored: unknown): Settings {
  const raw = (typeof stored === "object" && stored !== null ? stored : {}) as Record<
    string,
    unknown
  >;
  const list = (value: unknown, clean: (text: string) => string | undefined, fallback: string[]) =>
    Array.isArray(value)
      ? [
          ...new Set(
            value.flatMap((entry) => (typeof entry === "string" ? (clean(entry) ?? []) : [])),
          ),
        ]
      : fallback;
  const size = typeof raw.minSizeMb === "number" ? raw.minSizeMb : DEFAULT_SETTINGS.minSizeMb;
  return {
    autoCatch: typeof raw.autoCatch === "boolean" ? raw.autoCatch : DEFAULT_SETTINGS.autoCatch,
    minSizeMb: Number.isFinite(size)
      ? Math.min(100_000, Math.max(0, size))
      : DEFAULT_SETTINGS.minSizeMb,
    fileTypes: list(raw.fileTypes, cleanType, DEFAULT_SETTINGS.fileTypes),
    excludedSites: list(raw.excludedSites, cleanSite, DEFAULT_SETTINGS.excludedSites),
  };
}

export async function loadSettings(): Promise<Settings> {
  return normalise((await browser.storage.local.get(KEY))[KEY]);
}

export async function saveSettings(settings: Settings): Promise<void> {
  await browser.storage.local.set({ [KEY]: normalise(settings) });
}

/** Calls back whenever the rules change, from any page of the extension. */
export function onSettingsChanged(listener: (settings: Settings) => void): () => void {
  const handle = (changes: Record<string, { newValue?: unknown }>, area: string) => {
    if (area === "local" && changes[KEY]) listener(normalise(changes[KEY].newValue));
  };
  browser.storage.onChanged.addListener(handle);
  return () => browser.storage.onChanged.removeListener(handle);
}
