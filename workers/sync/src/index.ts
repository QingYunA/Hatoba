import { Hono } from "hono";
import { HTTPException } from "hono/http-exception";
import type { AppEnv } from "./env";
import { ApiError } from "./errors";
import { authRoutes } from "./routes/auth";
import { devicesRoutes } from "./routes/devices";
import { healthRoutes } from "./routes/health";
import { itemsRoutes } from "./routes/items";
import { setupRoutes } from "./routes/setup";
import { vaultRoutes } from "./routes/vault";

/**
 * Hatoba sync Worker. Stores only ciphertext. Clients are native (Rust) applications, so no
 * CORS headers are ever emitted and cross-origin browser access is deliberately impossible.
 */
const app = new Hono<AppEnv>();

// Responses carry tokens and ciphertext: never cache them anywhere.
app.use("*", async (c, next) => {
  await next();
  c.header("Cache-Control", "no-store");
  c.header("X-Content-Type-Options", "nosniff");
});

const v1 = new Hono<AppEnv>();
v1.route("/", healthRoutes);
v1.route("/", setupRoutes);
v1.route("/", authRoutes);
v1.route("/", vaultRoutes);
v1.route("/", itemsRoutes);
v1.route("/", devicesRoutes);
app.route("/v1", v1);

app.notFound((c) => c.json({ error: "not_found" }, 404));

app.onError((err, c) => {
  if (err instanceof ApiError) {
    for (const [name, value] of Object.entries(err.headers ?? {})) c.header(name, value);
    return c.json(err.hasMessage ? { error: err.code, message: err.message } : { error: err.code }, err.status);
  }
  if (err instanceof HTTPException) {
    return c.json({ error: "http_error", message: err.message }, err.status as 400);
  }
  // Log the error class only: messages from the runtime or D1 must never be able to echo request data.
  console.error("unhandled error", err instanceof Error ? err.name : typeof err);
  return c.json({ error: "internal_error" }, 500);
});

export default app;
