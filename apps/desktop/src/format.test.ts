import { describe, expect, it } from "vitest";
import { clock, duration, filledBlocks, formatSize, percent, speedMb, timeLeft } from "./format";

const MB = 1024 * 1024;

describe("formatSize", () => {
  it("uses the forms in the design", () => {
    expect(formatSize(0)).toBe("0 KB");
    expect(formatSize(640 * 1024)).toBe("640 KB");
    expect(formatSize(120 * MB)).toBe("120 MB");
    expect(formatSize(4.2 * MB)).toBe("4.2 MB");
    expect(formatSize(1.8 * 1024 * MB)).toBe("1.8 GB");
    expect(formatSize(1000 * MB)).toBe("1.0 GB");
  });
});

describe("speedMb", () => {
  it("has one decimal", () => {
    expect(speedMb(0)).toBe("0.0");
    expect(speedMb(8.1 * MB)).toBe("8.1");
  });
});

describe("timeLeft", () => {
  it("is MM:SS under an hour", () => {
    expect(timeLeft({ totalBytes: 235 * MB, completedBytes: 100 * MB, speedBps: MB })).toBe(
      "02:15",
    );
    expect(timeLeft({ totalBytes: 3725 * MB, completedBytes: 0, speedBps: MB })).toBe("1:02:05");
  });
  it("is unknown without a speed or a size", () => {
    expect(timeLeft({ totalBytes: 10, completedBytes: 0, speedBps: 0 })).toBeNull();
    expect(timeLeft({ totalBytes: 0, completedBytes: 0, speedBps: 5 })).toBeNull();
  });
});

describe("clock", () => {
  it("pads hours and minutes", () => {
    const at = new Date(2026, 0, 1, 9, 5).getTime() / 1000;
    expect(clock(at)).toBe("09:05");
  });
});

describe("percent", () => {
  it("is 0 for an unknown size and reaches 100 only when done", () => {
    expect(percent({ state: "active", totalBytes: 0, completedBytes: 10 })).toBe(0);
    expect(percent({ state: "active", totalBytes: 200, completedBytes: 50 })).toBe(25);
    expect(percent({ state: "paused", totalBytes: 1126, completedBytes: 120 })).toBe(11);
    expect(percent({ state: "active", totalBytes: 200, completedBytes: 200 })).toBe(99);
    expect(percent({ state: "done", totalBytes: 0, completedBytes: 0 })).toBe(100);
  });
});

describe("filledBlocks", () => {
  it("rounds, and always fills at least one block", () => {
    expect(filledBlocks(0, 32)).toBe(1);
    expect(filledBlocks(11, 32)).toBe(4);
    expect(filledBlocks(62, 28)).toBe(17);
    expect(filledBlocks(100, 32)).toBe(32);
  });
});

describe("duration", () => {
  it("reads like a clock", () => {
    expect(duration(6)).toBe("00:06");
    expect(duration(2538)).toBe("42:18");
    expect(duration(3735)).toBe("1:02:15");
  });
});
