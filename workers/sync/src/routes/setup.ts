import { Hono } from "hono";
import {
  MAX_BODY_BYTES_SMALL,
  MAX_KDF_PARAMS_BYTES,
  MAX_KEY_BLOB_BYTES,
  SECRET_KEY_BYTES,
} from "../config";
import type { AppEnv } from "../env";
import { ApiError } from "../errors";
import { bearerToken, limitBody, rateLimit } from "../middleware";
import { constantTimeEqual, sha256Hex } from "../util";
import {
  readBase64Secret,
  readInteger,
  readJsonBody,
  readKdfParams,
  readKdfSalt,
  readString,
} from "../validate";

export const setupRoutes = new Hono<AppEnv>();

/**
 * One-time vault initialisation. Guarded by the SETUP_TOKEN secret so nobody can claim a freshly
 * deployed, still unconfigured Worker. The client sends the raw `auth_key` and `recovery_auth`;
 * only their SHA-256 digests are stored.
 */
setupRoutes.post("/setup", rateLimit("setup"), limitBody(MAX_BODY_BYTES_SMALL), async (c) => {
  const expected = c.env.SETUP_TOKEN;
  if (!expected) {
    throw new ApiError(503, "setup_token_not_configured", "Run `wrangler secret put SETUP_TOKEN` first");
  }
  const provided = bearerToken(c.req.header("Authorization"));
  if (!provided || !(await constantTimeEqual(provided, expected))) {
    throw new ApiError(401, "invalid_setup_token", "Missing or incorrect setup token", {
      "WWW-Authenticate": "Bearer",
    });
  }

  const body = await readJsonBody(c);
  const schemaVersion = readInteger(body, "schema_version", { min: 1, max: 65535 });
  const kdfSalt = readKdfSalt(body);
  const kdfParams = readKdfParams(body, "kdf_params", MAX_KDF_PARAMS_BYTES);
  const authKey = readBase64Secret(body, "auth_key", SECRET_KEY_BYTES);
  const protectedVaultKey = readString(body, "protected_vault_key", { maxBytes: MAX_KEY_BLOB_BYTES });
  const recoveryVaultKey = readString(body, "recovery_vault_key", { maxBytes: MAX_KEY_BLOB_BYTES });
  const recoveryAuth = readBase64Secret(body, "recovery_auth", SECRET_KEY_BYTES);

  const result = await c.env.DB.prepare(
    `INSERT INTO meta (id, schema_version, kdf_salt, kdf_params, auth_hash, protected_vault_key,
                       recovery_vault_key, recovery_auth_hash, seq, created_at)
     VALUES (1, ?, ?, ?, ?, ?, ?, ?, 0, ?)
     ON CONFLICT(id) DO NOTHING`,
  )
    .bind(
      schemaVersion,
      kdfSalt,
      kdfParams,
      await sha256Hex(authKey),
      protectedVaultKey,
      recoveryVaultKey,
      await sha256Hex(recoveryAuth),
      Date.now(),
    )
    .run();

  if (result.meta.changes === 0) {
    throw new ApiError(409, "already_initialized", "This Worker already has a vault");
  }
  return c.json({ initialized: true }, 201);
});
