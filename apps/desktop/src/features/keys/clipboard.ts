import { isTauri } from "@/ipc/api";

/** Writes text to the system clipboard (Tauri plugin in the app, the web API in a browser). */
export async function copyText(text: string): Promise<void> {
  if (isTauri()) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
    return;
  }
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Insecure contexts / denied permission: fall back to a transient textarea.
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    if (!ok) throw new Error("clipboard unavailable");
  }
}
