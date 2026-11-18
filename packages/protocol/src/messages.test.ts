import { describe, expect, it } from "vitest";
import { ackSchema, downloadRequestSchema } from "./messages";

describe("downloadRequestSchema", () => {
  it("accepts http and https links only", () => {
    const base = { type: "download", version: 1 };
    expect(downloadRequestSchema.safeParse({ ...base, url: "https://host/a.zip" }).success).toBe(
      true,
    );
    expect(downloadRequestSchema.safeParse({ ...base, url: "magnet:?xt=urn:x" }).success).toBe(
      false,
    );
    expect(downloadRequestSchema.safeParse({ ...base, url: "not a link" }).success).toBe(false);
    expect(
      downloadRequestSchema.safeParse({ ...base, version: 2, url: "https://host/a" }).success,
    ).toBe(false);
  });
});

describe("ackSchema", () => {
  // These are the exact bytes the app and the host send.
  it("reads what the Rust side writes", () => {
    expect(ackSchema.parse(JSON.parse('{"type":"ack","ok":true,"id":"gid-1"}'))).toEqual({
      type: "ack",
      ok: true,
      id: "gid-1",
    });
    expect(
      ackSchema.parse(JSON.parse('{"type":"ack","ok":false,"code":"app_not_running"}')),
    ).toEqual({ type: "ack", ok: false, code: "app_not_running" });
  });

  it("refuses anything else", () => {
    expect(ackSchema.safeParse({ type: "ack", ok: false, code: "surprise" }).success).toBe(false);
    expect(ackSchema.safeParse({ type: "ack", ok: true }).success).toBe(false);
    expect(ackSchema.safeParse(undefined).success).toBe(false);
  });
});
