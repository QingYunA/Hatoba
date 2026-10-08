import type { HostFilter } from "@/app/store";

/** Host editor (design §02). Placeholder — implemented by the hosts feature. */
export function HostEditPage({ hostId }: { hostId: string | null; groupId: string | null; back: HostFilter }) {
  return <div style={{ padding: 24 }}>Edit {hostId ?? "new"}</div>;
}
