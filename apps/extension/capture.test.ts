import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS,
  cleanSite,
  cleanType,
  cookieHeader,
  displayName,
  extensionsOf,
  isExcluded,
  shouldCatch,
  siteOf,
  suggestedName,
  type DownloadLike,
  type Settings,
} from "./capture";

const MB = 1024 * 1024;
const item = (over: Partial<DownloadLike> = {}): DownloadLike => ({
  url: "https://host.example/file.zip",
  state: "in_progress",
  totalBytes: 50 * MB,
  ...over,
});
const rules = (over: Partial<Settings> = {}): Settings => ({ ...DEFAULT_SETTINGS, ...over });

describe("shouldCatch", () => {
  it("catches an ordinary download when catching is on", () => {
    expect(shouldCatch(item(), rules())).toBe(true);
    expect(shouldCatch(item(), rules({ autoCatch: false }))).toBe(false);
  });

  it("catches when the size is unknown, and leaves small files alone", () => {
    expect(shouldCatch(item({ totalBytes: -1 }), rules())).toBe(true);
    expect(shouldCatch(item({ totalBytes: 0 }), rules())).toBe(true);
    expect(shouldCatch(item({ totalBytes: 5 * MB - 1 }), rules())).toBe(false);
    expect(shouldCatch(item({ totalBytes: 5 * MB }), rules())).toBe(true);
    expect(shouldCatch(item({ totalBytes: 2 * MB }), rules({ minSizeMb: 1 }))).toBe(true);
  });

  it("goes by file type when the name shows one", () => {
    expect(shouldCatch(item({ url: "https://h.example/notes.txt" }), rules())).toBe(false);
    expect(shouldCatch(item({ url: "https://h.example/backup.tar.gz" }), rules())).toBe(true);
    expect(shouldCatch(item({ url: "https://h.example/download?id=42" }), rules())).toBe(true);
    // The browser's chosen name wins over the address.
    const named = item({
      url: "https://h.example/get?id=1",
      filename: "/Users/me/Downloads/a.txt",
    });
    expect(shouldCatch(named, rules())).toBe(false);
    expect(shouldCatch(named, rules({ fileTypes: ["txt"] }))).toBe(true);
  });

  it("never catches on an excluded site or its subdomains", () => {
    const excluded = rules({ excludedSites: ["example.com"] });
    const from = (referrer: string) => item({ url: "https://cdn.other.net/a.zip", referrer });
    expect(shouldCatch(from("https://drive.example.com/folder"), excluded)).toBe(false);
    expect(shouldCatch(from("https://www.example.com/"), excluded)).toBe(false);
    expect(shouldCatch(from("https://notexample.com/"), excluded)).toBe(true);
    // With no page known, the download's own host is the site.
    expect(shouldCatch(item({ url: "https://example.com/a.zip" }), excluded)).toBe(false);
  });

  it("leaves alone what it cannot or should not move", () => {
    expect(shouldCatch(item({ state: "complete" }), rules())).toBe(false);
    expect(shouldCatch(item({ state: "interrupted" }), rules())).toBe(false);
    expect(shouldCatch(item({ byExtensionId: "abc" }), rules())).toBe(false);
    expect(shouldCatch(item({ url: "blob:https://host/1234" }), rules())).toBe(false);
    expect(shouldCatch(item({ url: "data:text/plain,hi" }), rules())).toBe(false);
    expect(shouldCatch(item({ url: "https://a/x", finalUrl: "blob:https://a/1" }), rules())).toBe(
      false,
    );
  });
});

describe("names and types", () => {
  it("keeps only the file name", () => {
    expect(suggestedName(item({ filename: "/Users/me/Downloads/a.zip" }))).toBe("a.zip");
    expect(suggestedName(item({ filename: "C:\\Users\\me\\a.zip" }))).toBe("a.zip");
    expect(suggestedName(item({ filename: "" }))).toBeUndefined();
  });

  it("falls back to the end of the address", () => {
    expect(displayName(item({ url: "https://h.example/files/My%20File.zip?x=1" }))).toBe(
      "My File.zip",
    );
    expect(displayName(item({ url: "https://h.example/" }))).toBe("h.example");
  });

  it("reads double extensions", () => {
    expect(extensionsOf("backup.tar.gz")).toEqual(["tar.gz", "gz"]);
    expect(extensionsOf("photo.JPG")).toEqual(["jpg"]);
    expect(extensionsOf("README")).toEqual([]);
    expect(extensionsOf("node-v22.11.0.pkg")).toContain("pkg");
  });
});

describe("what the user types into the lists", () => {
  it("tidies a site", () => {
    expect(cleanSite(" https://Drive.Example.com/folder ")).toBe("drive.example.com");
    expect(cleanSite("www.example.org")).toBe("example.org");
    expect(cleanSite("not a site")).toBeUndefined();
    expect(cleanSite("localhost")).toBeUndefined();
  });

  it("tidies a file type", () => {
    expect(cleanType(".ZIP")).toBe("zip");
    expect(cleanType("tar.gz")).toBe("tar.gz");
    expect(cleanType("a b")).toBeUndefined();
    expect(cleanType("")).toBeUndefined();
  });

  it("matches sites by name and subdomain", () => {
    expect(siteOf("https://www.Example.com/x")).toBe("example.com");
    expect(siteOf("chrome://extensions")).toBeUndefined();
    expect(isExcluded("a.example.com", ["example.com"])).toBe(true);
    expect(isExcluded("example.com.evil.net", ["example.com"])).toBe(false);
    expect(isExcluded(undefined, ["example.com"])).toBe(false);
  });
});

describe("cookieHeader", () => {
  it("joins cookies and returns nothing for none or too many", () => {
    expect(
      cookieHeader([
        { name: "a", value: "1" },
        { name: "b", value: "2" },
      ]),
    ).toBe("a=1; b=2");
    expect(cookieHeader([])).toBeUndefined();
    expect(cookieHeader([{ name: "big", value: "x".repeat(40_000) }])).toBeUndefined();
  });
});
