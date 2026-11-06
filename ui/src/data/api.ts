import type { Doc, Workspace } from "./types";
import { mockDocs, mockWorkspace } from "./mock";

// Without the Rust core (plain `vite dev`), the UI falls back to the mock workspace.
let offline = false;

export async function loadWorkspace(): Promise<{ workspace: Workspace; offline: boolean }> {
  try {
    const r = await fetch("/api/workspace");
    if (!r.ok) throw new Error(String(r.status));
    offline = false;
    return { workspace: await r.json(), offline };
  } catch {
    offline = true;
    return { workspace: mockWorkspace, offline };
  }
}

export async function loadDoc(id: string): Promise<Doc | null> {
  if (offline) return mockDocs[id] ?? null;
  const r = await fetch(`/api/doc/${encodeURIComponent(id)}`);
  return r.ok ? r.json() : null;
}

export async function markRead(id: string): Promise<void> {
  if (offline) return;
  // The custom header is required by the core so other sites cannot forge writes.
  await fetch(`/api/read/${encodeURIComponent(id)}`, { method: "POST", headers: { "X-Mieruka": "1" } });
}

const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

export function formatReadAt(ms: number): string {
  const minutes = Math.round((ms - Date.now()) / 60000);
  if (Math.abs(minutes) < 60) return rtf.format(minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return rtf.format(hours, "hour");
  return rtf.format(Math.round(hours / 24), "day");
}
