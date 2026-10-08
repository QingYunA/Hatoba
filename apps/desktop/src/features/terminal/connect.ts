import { hostById } from "@/app/data";
import { useApp } from "@/app/store";
import { useTabs } from "@/app/tabs";
import { toast } from "@/components/overlay";
import { t } from "@/i18n";
import type { HostView } from "@/ipc/types";
import { startSessionListeners } from "./listeners";
import { askSecret } from "./prompts";
import { ensureSession, getSession, type Credentials } from "./session";

const NONE: Credentials = { password: null, passphrase: null };

/**
 * Ask for whatever the host's auth mode needs before connecting. `null` means the user cancelled.
 * "Ask every time" hosts get a password prompt (SSH-03); key passphrases are asked lazily, only if
 * the backend reports it has none (see LiveSession.connect).
 */
export async function collectCredentials(host: HostView): Promise<Credentials | null> {
  if (host.auth_kind !== "ask") return NONE;
  const password = await askSecret({
    kind: "password",
    hostName: host.name,
    target: `${host.username}@${host.address}:${host.port}`,
  });
  return password === null ? null : { password, passphrase: null };
}

/** Open a new terminal tab for a host and connect (HOST-07, TERM-01: the same host may have several tabs). */
export async function connectHost(hostId: string): Promise<void> {
  startSessionListeners();
  const host = hostById(hostId);
  if (!host) {
    toast(t("terminal.hostMissing"), "error");
    return;
  }
  const creds = await collectCredentials(host);
  if (!creds) return;
  const tabId = useTabs.getState().openSession(hostId, host.name);
  const session = ensureSession(tabId, hostId);
  // The view connects as soon as it has measured the terminal size.
  session?.setCredentials(creds);
}

/** Retry / reconnect a tab in place (prompting again for "ask every time" hosts). */
export async function reconnectSession(tabId: string): Promise<void> {
  const session = getSession(tabId);
  const host = session ? hostById(session.hostId) : undefined;
  if (!session) return;
  await session.reconnect(async () => (host ? collectCredentials(host) : NONE));
}

/** Disconnect (if needed) and close a session tab. */
export async function closeSessionTab(tabId: string): Promise<void> {
  const session = getSession(tabId);
  useTabs.getState().closeTab(tabId);
  await session?.dispose();
}

/** Jump to the host's edit page (from the error card / menu). */
export function editSessionHost(hostId: string) {
  const host = hostById(hostId);
  if (!host) return;
  useApp.getState().navigate({ kind: "host-edit", hostId, groupId: host.group_id, back: { kind: "all" } });
  useTabs.getState().activate("home");
}
