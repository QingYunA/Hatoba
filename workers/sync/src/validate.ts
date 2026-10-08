import type { Context } from "hono";
import { ApiError } from "./errors";
import { base64ToBytes, exceedsBytes } from "./util";

export type JsonObject = Record<string, unknown>;

/** Item and device identifiers: UUIDs (any version) or the literal "settings". */
export const ID_PATTERN = /^[A-Za-z0-9_-]{1,64}$/;
/** Opaque salt token (base64, base64url or hex are all accepted; the server never decodes it). */
const SALT_PATTERN = /^[A-Za-z0-9+/_=-]{16,256}$/;

/** Builds a 400 that names the offending field but never echoes its value. */
export function invalid(field: string, problem: string): ApiError {
  return new ApiError(400, "invalid_request", `${field}: ${problem}`);
}

export function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Parses the request body as a JSON object; 415 for other media types, 400 for bad JSON. */
export async function readJsonBody(c: Context): Promise<JsonObject> {
  const contentType = c.req.header("Content-Type") ?? "";
  if (!/^application\/json\s*(;|$)/i.test(contentType)) {
    throw new ApiError(415, "unsupported_media_type", "Content-Type must be application/json");
  }
  let parsed: unknown;
  try {
    parsed = await c.req.json();
  } catch {
    throw new ApiError(400, "invalid_json", "Request body is not valid JSON");
  }
  if (!isObject(parsed)) throw invalid("body", "must be a JSON object");
  return parsed;
}

export function readString(
  obj: JsonObject,
  key: string,
  opts: { maxBytes: number; minLength?: number; pattern?: RegExp },
): string {
  const value = obj[key];
  if (typeof value !== "string") throw invalid(key, "must be a string");
  if (value.length < (opts.minLength ?? 1)) throw invalid(key, "is too short");
  if (exceedsBytes(value, opts.maxBytes)) throw invalid(key, `must be at most ${opts.maxBytes} bytes`);
  if (opts.pattern && !opts.pattern.test(value)) throw invalid(key, "has an invalid format");
  return value;
}

export function readInteger(obj: JsonObject, key: string, opts: { min: number; max?: number }): number {
  const value = obj[key];
  if (typeof value !== "number" || !Number.isSafeInteger(value)) throw invalid(key, "must be an integer");
  if (value < opts.min || (opts.max !== undefined && value > opts.max)) throw invalid(key, "is out of range");
  return value;
}

export function readBoolean(obj: JsonObject, key: string): boolean {
  const value = obj[key];
  if (typeof value !== "boolean") throw invalid(key, "must be a boolean");
  return value;
}

export function readId(obj: JsonObject, key: string): string {
  return readString(obj, key, { maxBytes: 64, pattern: ID_PATTERN });
}

/** A secret sent as standard base64 that must decode to exactly `length` bytes. */
export function readBase64Secret(obj: JsonObject, key: string, length: number): Uint8Array {
  const value = obj[key];
  if (typeof value !== "string") throw invalid(key, "must be a base64 string");
  const bytes = base64ToBytes(value);
  if (!bytes || bytes.length !== length) throw invalid(key, `must be standard base64 encoding of ${length} bytes`);
  return bytes;
}

export function readKdfSalt(obj: JsonObject, key = "kdf_salt"): string {
  return readString(obj, key, { maxBytes: 256, minLength: 16, pattern: SALT_PATTERN });
}

/** KDF parameters are kept as the raw JSON string the client sent; it must be a JSON object. */
export function readKdfParams(obj: JsonObject, key: string, maxBytes: number): string {
  const value = readString(obj, key, { maxBytes });
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch {
    throw invalid(key, "must be a JSON document encoded as a string");
  }
  if (!isObject(parsed)) throw invalid(key, "must encode a JSON object");
  return value;
}

export function readDevice(obj: JsonObject, maxNameBytes: number): { deviceId: string; deviceName: string } {
  return {
    deviceId: readId(obj, "device_id"),
    deviceName: readString(obj, "device_name", { maxBytes: maxNameBytes }),
  };
}
