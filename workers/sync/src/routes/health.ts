import { Hono } from "hono";
import { API_VERSION, SERVICE_NAME, VERSION } from "../config";
import type { AppEnv } from "../env";
import { ApiError, notInitialized } from "../errors";

export const healthRoutes = new Hono<AppEnv>();

/** Unauthenticated: service identity and whether /v1/setup has already been run. */
healthRoutes.get("/health", async (c) => {
  let initialized: boolean;
  try {
    initialized = (await c.env.DB.prepare("SELECT 1 AS present FROM meta WHERE id = 1").first()) !== null;
  } catch (err) {
    console.error("health: database check failed", err instanceof Error ? err.name : "unknown");
    throw new ApiError(503, "database_unavailable", "D1 is unreachable or migrations have not been applied");
  }
  return c.json({ service: SERVICE_NAME, version: VERSION, api: API_VERSION, initialized });
});

/** Unauthenticated: the client needs salt and KDF parameters before it can derive `auth_key`. */
healthRoutes.get("/prelogin", async (c) => {
  const meta = await c.env.DB.prepare("SELECT kdf_salt, kdf_params FROM meta WHERE id = 1").first<{
    kdf_salt: string;
    kdf_params: string;
  }>();
  if (!meta) throw notInitialized();
  // kdf_params is returned as the raw JSON string exactly as stored.
  return c.json({ kdf_salt: meta.kdf_salt, kdf_params: meta.kdf_params });
});
