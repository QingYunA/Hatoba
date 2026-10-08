import type { SessionTab } from "@/app/tabs";

/** One terminal session (design §03 / §03b). Placeholder — implemented by the terminal feature. */
export function TerminalView({ tab, active }: { tab: SessionTab; active: boolean }) {
  return <div style={{ display: active ? "flex" : "none", flex: 1, background: "var(--term)" }}>{tab.title}</div>;
}
