import type { SessionScope } from "./config";

/** Cloudflare bindings and secrets available to the Worker. */
export interface Bindings {
  DB: D1Database;
  /** Secret set with `wrangler secret put SETUP_TOKEN`; absent until configured. */
  SETUP_TOKEN?: string;
  /** Workers Rate Limiting binding; absent in some local setups, in which case limiting is skipped. */
  AUTH_LIMITER?: RateLimit;
}

export interface SessionInfo {
  tokenHash: string;
  deviceId: string;
  scope: SessionScope;
}

export interface AppEnv {
  Bindings: Bindings;
  Variables: { session: SessionInfo };
}
