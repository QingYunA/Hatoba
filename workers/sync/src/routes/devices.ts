import { Hono } from "hono";
import type { AppEnv } from "../env";
import { requireSession } from "../middleware";
import { ID_PATTERN, invalid } from "../validate";

export const devicesRoutes = new Hono<AppEnv>();

interface DeviceRow {
  device_id: string;
  device_name: string;
  created_at: number;
  last_seen: number;
  expires_at: number;
}

/** Devices are the unexpired full sessions; `device_name` is returned as the opaque envelope it was stored as. */
devicesRoutes.get("/devices", requireSession(), async (c) => {
  const { results } = await c.env.DB.prepare(
    `SELECT device_id, device_name, created_at, last_seen, expires_at
     FROM sessions WHERE scope = 'full' AND expires_at > ? ORDER BY last_seen DESC`,
  )
    .bind(Date.now())
    .all<DeviceRow>();

  const currentDeviceId = c.get("session").deviceId;
  return c.json({ devices: results.map((row) => ({ ...row, current: row.device_id === currentDeviceId })) });
});

/** Revokes every session of a device (including the caller's own device). Idempotent. */
devicesRoutes.delete("/devices/:device_id", requireSession(), async (c) => {
  const deviceId = c.req.param("device_id");
  if (!ID_PATTERN.test(deviceId)) throw invalid("device_id", "has an invalid format");
  await c.env.DB.prepare("DELETE FROM sessions WHERE device_id = ?").bind(deviceId).run();
  return c.body(null, 204);
});
