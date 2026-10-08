import { describe, expect, it } from "vitest";
import { VERSION } from "../src/config";
import { base64ToBytes, bytesToBase64Url, constantTimeEqual, exceedsBytes, sha256Hex } from "../src/util";
import pkg from "../package.json";

describe("util", () => {
  it("keeps VERSION in sync with package.json", () => {
    expect(VERSION).toBe(pkg.version);
  });

  it("decodes only canonical padded standard base64", () => {
    expect(base64ToBytes("AQID")).toEqual(new Uint8Array([1, 2, 3]));
    expect(base64ToBytes("AQI=")).toEqual(new Uint8Array([1, 2]));
    expect(base64ToBytes("")).toEqual(new Uint8Array());
    expect(base64ToBytes("AQI")).toBeNull(); // missing padding
    expect(base64ToBytes("AQJ=")).toBeNull(); // non-canonical trailing bits
    expect(base64ToBytes("AQ-_")).toBeNull(); // url-safe alphabet
    expect(base64ToBytes("AQ I=")).toBeNull();
    expect(base64ToBytes("A===")).toBeNull();
  });

  it("encodes base64url without padding", () => {
    expect(bytesToBase64Url(new Uint8Array([251, 255, 254]))).toBe("-__-");
    expect(bytesToBase64Url(new Uint8Array([1]))).toBe("AQ");
  });

  it("hashes with SHA-256 (hex)", async () => {
    expect(await sha256Hex("abc")).toBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
  });

  it("compares strings in constant time regardless of length", async () => {
    expect(await constantTimeEqual("secret", "secret")).toBe(true);
    expect(await constantTimeEqual("secret", "secreT")).toBe(false);
    expect(await constantTimeEqual("secret", "secret-longer")).toBe(false);
    expect(await constantTimeEqual("", "x")).toBe(false);
  });

  it("measures UTF-8 byte length", () => {
    expect(exceedsBytes("abcd", 4)).toBe(false);
    expect(exceedsBytes("abcde", 4)).toBe(true);
    expect(exceedsBytes("éé", 4)).toBe(false); // 2 x 2 bytes
    expect(exceedsBytes("ééé", 4)).toBe(true); // 6 bytes
    expect(exceedsBytes("€", 3)).toBe(false); // 3 bytes
    expect(exceedsBytes("€", 2)).toBe(true);
  });
});
