import type { KeyView } from "@/ipc/types";

/** "ED25519", "RSA 4096", "ECDSA P-256": the type badge in the list. */
export function keyTypeLabel(k: Pick<KeyView, "algorithm" | "bits">): string {
  switch (k.algorithm) {
    case "ed25519":
      return "ED25519";
    case "rsa":
      return `RSA ${k.bits}`;
    case "ecdsa":
      return `ECDSA P-${k.bits}`;
  }
}

/** Algorithm name without the size, for "ED25519 · 256 位". */
export function keyAlgoName(algorithm: KeyView["algorithm"]): string {
  return algorithm.toUpperCase();
}

/** "SHA256:4f9KxPq2mW…TkVfXw": keeps the prefix and both ends readable. */
export function shortFingerprint(fp: string): string {
  const colon = fp.indexOf(":");
  const body = fp.slice(colon + 1);
  if (body.length <= 18) return fp;
  return `${fp.slice(0, colon + 1)}${body.slice(0, 10)}…${body.slice(-6)}`;
}
