import { isTauri } from "@/ipc/api";

/** Copy text through the Tauri clipboard plugin, or the browser clipboard when running in a plain browser. */
export async function copyText(text: string): Promise<void> {
  if (isTauri()) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
    return;
  }
  await navigator.clipboard.writeText(text);
}

/**
 * Native "save as" dialog; resolves to the chosen path or `null` when cancelled. In a plain browser there is no
 * dialog, so the suggested name is returned (the mock backend turns it into a download).
 */
export async function pickSavePath(defaultName: string, filter: { name: string; extension: string }): Promise<string | null> {
  if (!isTauri()) return defaultName;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ defaultPath: defaultName, filters: [{ name: filter.name, extensions: [filter.extension] }] });
}
