import type { useT } from "@/i18n";
import { api } from "@/ipc/api";
import { useApp } from "@/app/store";
import type { DeviceView, SyncStatus } from "@/ipc/types";

export type T = ReturnType<typeof useT>;

export const STALE_DEVICE_MS = 14 * 86_400_000;

/** Fetches the latest status into the app store (events normally do this; used after our own commands). */
export async function refreshSyncStatus(): Promise<SyncStatus> {
  const status = await api.sync_status();
  useApp.getState().setSync(status);
  return status;
}

export function isStale(d: DeviceView, now = Date.now()): boolean {
  return !d.current && now - d.last_seen > STALE_DEVICE_MS;
}

/** Laptop by default; a few name/platform hints switch to desktop or phone. */
export function deviceIcon(d: Pick<DeviceView, "name" | "platform">): string {
  if (/android|ios|ipad|iphone/i.test(d.platform)) return "device-mobile";
  if (/mini|desktop|imac|studio|tower|workstation|server|-ws\b|nas\b/i.test(d.name)) return "desktop";
  return "laptop";
}

/** "11 台主机、5 把密钥、5 个分组" */
export function countsLine(t: T, counts: NonNullable<SyncStatus["counts"]>): string {
  return [
    t("sync.count.hosts", { n: counts.hosts }),
    t("sync.count.keys", { n: counts.keys }),
    t("sync.count.groups", { n: counts.groups }),
  ].join(t("sync.count.sep"));
}
