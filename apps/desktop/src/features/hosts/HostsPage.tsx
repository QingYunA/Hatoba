import type { HostFilter } from "@/app/store";

/** Host list (design §01 / §01b). Placeholder — implemented by the hosts feature. */
export function HostsPage({ filter }: { filter: HostFilter }) {
  return <div style={{ padding: 24 }}>Hosts: {filter.kind}</div>;
}
