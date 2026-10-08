const encoder = new TextEncoder();

// --- encoding -------------------------------------------------------------

export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

export function bytesToBase64Url(bytes: Uint8Array): string {
  return bytesToBase64(bytes).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function bytesToHex(bytes: Uint8Array): string {
  let hex = "";
  for (const byte of bytes) hex += byte.toString(16).padStart(2, "0");
  return hex;
}

const BASE64_RE = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;

/**
 * Strict RFC 4648 section 4 decoder: standard alphabet, mandatory padding and canonical
 * encoding (re-encoding must reproduce the input). Returns null for anything else.
 */
export function base64ToBytes(value: string): Uint8Array | null {
  if (!BASE64_RE.test(value)) return null;
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytesToBase64(bytes) === value ? bytes : null;
}

// --- hashing and randomness ----------------------------------------------

export async function sha256(data: Uint8Array | string): Promise<Uint8Array> {
  const input = typeof data === "string" ? encoder.encode(data) : data;
  return new Uint8Array(await crypto.subtle.digest("SHA-256", input));
}

export async function sha256Hex(data: Uint8Array | string): Promise<string> {
  return bytesToHex(await sha256(data));
}

export function randomBytes(length: number): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(length));
}

/**
 * Constant-time string comparison. Both inputs are hashed first so the comparison always runs
 * over two 32-byte digests, which also hides any length difference.
 */
export async function constantTimeEqual(a: string, b: string): Promise<boolean> {
  const [digestA, digestB] = await Promise.all([sha256(a), sha256(b)]);
  return crypto.subtle.timingSafeEqual(digestA, digestB);
}

// --- sizes ----------------------------------------------------------------

/** True when `value` is longer than `maxBytes` once encoded as UTF-8. */
export function exceedsBytes(value: string, maxBytes: number): boolean {
  // UTF-8 never uses fewer bytes than UTF-16 code units, nor more than three per unit.
  if (value.length > maxBytes) return true;
  if (value.length * 3 <= maxBytes) return false;
  return encoder.encode(value).length > maxBytes;
}
