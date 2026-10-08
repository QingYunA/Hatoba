import type { AppInfo } from "@/ipc/types";

export type Platform = AppInfo["platform"];

/** Windows / Linux use Ctrl+Shift app shortcuts (WIN-04); macOS uses ⌘. */
export function isMac(platform: Platform): boolean {
  return platform === "macos";
}

/** Human-readable shortcut label, e.g. shortcutLabel("Shift+K") → "Ctrl+Shift+K" or "⌘K". */
export function shortcutLabel(platform: Platform, win: string, mac: string): string {
  return isMac(platform) ? mac : win;
}
