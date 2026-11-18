import { describe, expect, it } from "vitest";
import { formatBytes, formatSpeed, percent } from "./format";

describe("formatBytes", () => {
  it("keeps bytes whole and scales up with one decimal", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1023 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1.8 * 1024 ** 3)).toBe("1.8 GB");
  });
});

describe("formatSpeed", () => {
  it("reads in MB/s", () => {
    expect(formatSpeed(0)).toBe("0.0 MB/s");
    expect(formatSpeed(8.1 * 1024 * 1024)).toBe("8.1 MB/s");
  });
});

describe("percent", () => {
  it("is 0 for an unknown size and never passes 100", () => {
    expect(percent(10, 0)).toBe(0);
    expect(percent(50, 200)).toBe(25);
    expect(percent(300, 200)).toBe(100);
    expect(percent(199, 200)).toBe(99);
  });
});
