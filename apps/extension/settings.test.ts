import { describe, expect, it, vi } from "vitest";

vi.mock("wxt/browser", () => ({ browser: {} }));

import { DEFAULT_SETTINGS } from "./capture";
import { normalise } from "./settings";

describe("normalise", () => {
  it("falls back to the defaults for nothing or nonsense", () => {
    expect(normalise(undefined)).toEqual(DEFAULT_SETTINGS);
    expect(normalise("x")).toEqual(DEFAULT_SETTINGS);
    expect(normalise({ autoCatch: "yes", minSizeMb: "5" })).toEqual(DEFAULT_SETTINGS);
  });

  it("keeps what is valid and drops what is not", () => {
    const stored = {
      autoCatch: false,
      minSizeMb: -3,
      fileTypes: [".ZIP", "zip", "bad type", 7],
      excludedSites: ["https://Drive.Example.com/x", "nope"],
    };
    expect(normalise(stored)).toEqual({
      autoCatch: false,
      minSizeMb: 0,
      fileTypes: ["zip"],
      excludedSites: ["drive.example.com"],
    });
  });
});
