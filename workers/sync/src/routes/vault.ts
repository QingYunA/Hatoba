import { Hono } from "hono";
import {
  MAX_BODY_BYTES_SMALL,
  MAX_KDF_PARAMS_BYTES,
  MAX_KEY_BLOB_BYTES,
  SECRET_KEY_BYTES,
} from "../config";
import type { AppEnv } from "../env";
import { notInitialized } from "../errors";
import { limitBody, requireSession } from "../middleware";
import { sha256Hex } from "../util";
import {
  readBase64Secret,
  readJsonBody,
  readKdfParams,
  readKdfSalt,
  readString,
} from "../validate";

export const vaultRoutes = new Hono<AppEnv>();

vaultRoutes.get("/vault", requireSession(), async (c) => {
  const meta = await c.env.DB.prepare(
    `SELECT schema_version, kdf_salt, kdf_params, protected_vault_key, recovery_vault_key, seq
     FROM meta WHERE id = 1`,
  ).first<{
    schema_version: number;
    kdf_salt: string;
    kdf_params: string;
    protected_vault_key: string;
    recovery_vault_key: string;
    seq: number;
  }>();
  if (!meta) throw notInitialized();
  return c.json(meta);
});

/**
 * Changes the master password. Accepts a full session or the restricted session from
 * /v1/recover. The meta update and the session revocation run in one D1 batch (a transaction).
 *
 * - full session: every other session is revoked, the caller stays logged in.
 * - recovery session: every session including the caller's is revoked; the client must log in
 *   again with the new password.
 *
 * `recovery_vault_key` and `recovery_auth` rotate the recovery code and must be sent together.
 */
vaultRoutes.put(
  "/vault/password",
  requireSession(["full", "recovery"]),
  limitBody(MAX_BODY_BYTES_SMALL),
  async (c) => {
    const body = await readJsonBody(c);
    const kdfSalt = readKdfSalt(body);
    const kdfParams = readKdfParams(body, "kdf_params", MAX_KDF_PARAMS_BYTES);
    const authKey = readBase64Secret(body, "auth_key", SECRET_KEY_BYTES);
    const protectedVaultKey = readString(body, "protected_vault_key", { maxBytes: MAX_KEY_BLOB_BYTES });

    // Recovery material rotates as a pair: a new recovery code changes both values.
    const rotatingRecovery = body.recovery_vault_key !== undefined || body.recovery_auth !== undefined;
    const recovery = rotatingRecovery
      ? {
          vaultKey: readString(body, "recovery_vault_key", { maxBytes: MAX_KEY_BLOB_BYTES }),
          authHash: await sha256Hex(readBase64Secret(body, "recovery_auth", SECRET_KEY_BYTES)),
        }
      : null;

    const { tokenHash, scope } = c.get("session");
    const db = c.env.DB;
    const authHash = await sha256Hex(authKey);

    const updateMeta = recovery
      ? db
          .prepare(
            `UPDATE meta SET kdf_salt = ?, kdf_params = ?, auth_hash = ?, protected_vault_key = ?,
                            recovery_vault_key = ?, recovery_auth_hash = ?
             WHERE id = 1`,
          )
          .bind(kdfSalt, kdfParams, authHash, protectedVaultKey, recovery.vaultKey, recovery.authHash)
      : db
          .prepare(
            `UPDATE meta SET kdf_salt = ?, kdf_params = ?, auth_hash = ?, protected_vault_key = ?
             WHERE id = 1`,
          )
          .bind(kdfSalt, kdfParams, authHash, protectedVaultKey);

    const revokeSessions =
      scope === "full"
        ? db.prepare("DELETE FROM sessions WHERE token_hash != ?").bind(tokenHash)
        : db.prepare("DELETE FROM sessions");

    const [metaResult] = await db.batch([updateMeta, revokeSessions]);
    if (!metaResult || metaResult.meta.changes === 0) throw notInitialized();

    return c.json({ ok: true, relogin_required: scope === "recovery" });
  },
);
