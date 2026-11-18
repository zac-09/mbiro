import { describe, expect, it } from "vitest";
import {
  MIN_CATCH_BYTES,
  cookieHeader,
  shouldCatch,
  suggestedName,
  type DownloadLike,
} from "./capture";

const item = (over: Partial<DownloadLike> = {}): DownloadLike => ({
  url: "https://host/file.zip",
  state: "in_progress",
  totalBytes: 50 * MIN_CATCH_BYTES,
  ...over,
});

describe("shouldCatch", () => {
  it("catches an ordinary download when catching is on", () => {
    expect(shouldCatch(item(), true)).toBe(true);
    expect(shouldCatch(item(), false)).toBe(false);
  });

  it("catches when the size is unknown, and leaves small files alone", () => {
    expect(shouldCatch(item({ totalBytes: -1 }), true)).toBe(true);
    expect(shouldCatch(item({ totalBytes: 0 }), true)).toBe(true);
    expect(shouldCatch(item({ totalBytes: MIN_CATCH_BYTES - 1 }), true)).toBe(false);
    expect(shouldCatch(item({ totalBytes: MIN_CATCH_BYTES }), true)).toBe(true);
  });

  it("leaves alone what it cannot or should not move", () => {
    expect(shouldCatch(item({ state: "complete" }), true)).toBe(false);
    expect(shouldCatch(item({ state: "interrupted" }), true)).toBe(false);
    expect(shouldCatch(item({ byExtensionId: "abc" }), true)).toBe(false);
    expect(shouldCatch(item({ url: "blob:https://host/1234" }), true)).toBe(false);
    expect(shouldCatch(item({ url: "data:text/plain,hi" }), true)).toBe(false);
  });

  it("judges the address after redirects when the browser gives one", () => {
    expect(shouldCatch(item({ url: "https://a/x", finalUrl: "blob:https://a/1" }), true)).toBe(
      false,
    );
  });
});

describe("suggestedName", () => {
  it("keeps only the file name", () => {
    expect(suggestedName(item({ filename: "/Users/me/Downloads/a.zip" }))).toBe("a.zip");
    expect(suggestedName(item({ filename: "C:\\Users\\me\\a.zip" }))).toBe("a.zip");
    expect(suggestedName(item({ filename: "" }))).toBeUndefined();
    expect(suggestedName(item())).toBeUndefined();
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
