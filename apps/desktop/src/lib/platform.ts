import type { AppInfo } from "@/ipc/types";

export type Platform = AppInfo["platform"];

/** Windows / Linux use Ctrl+Shift app shortcuts (WIN-04); macOS uses ⌘. */
export function isMac(platform: Platform): boolean {
  return platform === "macos";
}

/** The terminal font a new vault starts with. Keep in step with `DEFAULT_FONT_FAMILY` in hatoba-core. */
export function defaultTerminalFont(platform: Platform): string {
  return isMac(platform) ? "Menlo" : "Cascadia Mono";
}

/** Human-readable shortcut label, e.g. shortcutLabel("Shift+K") → "Ctrl+Shift+K" or "⌘K". */
export function shortcutLabel(platform: Platform, win: string, mac: string): string {
  return isMac(platform) ? mac : win;
}
