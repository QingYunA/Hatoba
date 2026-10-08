import { useTabs } from "@/app/tabs";
import { hostById } from "@/app/data";

/** Open a new terminal tab for a host (HOST-07). Placeholder — implemented by the terminal feature. */
export async function connectHost(hostId: string): Promise<void> {
  const host = hostById(hostId);
  useTabs.getState().openSession(hostId, host?.name ?? hostId);
}

/** Disconnect (if needed) and close a session tab. */
export async function closeSessionTab(tabId: string): Promise<void> {
  useTabs.getState().closeTab(tabId);
}
