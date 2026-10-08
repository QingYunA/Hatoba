import type { D1Database, SyncTestResult } from "@/ipc/types";

export type Method = "worker" | "d1";

/** Progress of a connection test or token verification. */
export type Probe =
  | { status: "idle" }
  | { status: "testing" }
  | { status: "ok"; result: SyncTestResult }
  | { status: "failed"; message: string; offline?: boolean };

export interface WorkerForm {
  url: string;
  token: string;
  test: Probe;
}

export interface D1Form {
  accountId: string;
  apiToken: string;
  databaseId: string;
  databases: D1Database[];
  /** Token verification. */
  verify: Probe;
  /** The chosen database already holds a vault (flow C is not in the MVP). */
  initialized: boolean;
  /** Checking the chosen database for an existing vault. */
  checking: boolean;
}

export const EMPTY_WORKER: WorkerForm = { url: "", token: "", test: { status: "idle" } };
export const EMPTY_D1: D1Form = {
  accountId: "",
  apiToken: "",
  databaseId: "",
  databases: [],
  verify: { status: "idle" },
  initialized: false,
  checking: false,
};

/** "hatoba-sync.me.workers.dev/" → "https://hatoba-sync.me.workers.dev" */
export function normalizeWorkerUrl(raw: string): string {
  const trimmed = raw.trim().replace(/\/+$/, "");
  if (!trimmed) return "";
  return /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
}
