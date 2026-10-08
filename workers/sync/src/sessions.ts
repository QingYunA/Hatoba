import { RECOVERY_SESSION_TTL_MS, SESSION_TOKEN_BYTES, SESSION_TTL_MS, type SessionScope } from "./config";
import { bytesToBase64Url, randomBytes, sha256Hex } from "./util";

export interface NewSession {
  token: string;
  expiresAt: number;
}

/**
 * Creates a session and returns the bearer token (shown to the client exactly once).
 * Only the SHA-256 of the token is persisted. In the same transaction, expired sessions are
 * purged and any older session of the same device and scope is replaced, so a device has at
 * most one active session.
 */
export async function createSession(
  db: D1Database,
  input: { deviceId: string; deviceName: string; scope: SessionScope },
): Promise<NewSession> {
  const token = bytesToBase64Url(randomBytes(SESSION_TOKEN_BYTES));
  const tokenHash = await sha256Hex(token);
  const now = Date.now();
  const expiresAt = now + (input.scope === "full" ? SESSION_TTL_MS : RECOVERY_SESSION_TTL_MS);

  await db.batch([
    db.prepare("DELETE FROM sessions WHERE expires_at <= ?").bind(now),
    db.prepare("DELETE FROM sessions WHERE device_id = ? AND scope = ?").bind(input.deviceId, input.scope),
    db
      .prepare(
        `INSERT INTO sessions (token_hash, device_id, device_name, scope, created_at, last_seen, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(tokenHash, input.deviceId, input.deviceName, input.scope, now, now, expiresAt),
  ]);

  return { token, expiresAt };
}
