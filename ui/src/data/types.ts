// Shapes served by the Rust core (src/model.rs).

export type DocKind = "instructions" | "plans" | "notes";
export type Severity = "error" | "warning" | "info";

export interface Issue {
  id: string;
  rule: string; // e.g. "broken-import", "stale-path"
  title: string; // short label shown in bold
  message: string; // what is wrong and how to fix it
  severity: Severity;
  docId: string;
  blockId: string;
  line: number;
}

export interface Block {
  id: string;
  // Rendered, escaped HTML for one block. Issue ranges are wrapped in <mark data-issue="id">.
  html: string;
  change?: "changed" | "unreviewed";
  // Unreviewed blocks of one hunk are accepted or reverted together.
  hunk?: string;
  // Last block of a hunk: what the hunk looked like when last approved.
  previousHtml?: string;
  changedAt?: number; // unix ms
}

export interface DocMeta {
  id: string;
  projectId: string;
  path: string;
  kind: DocKind;
  lines: number;
  title: string;
  changes: number; // blocks changed since the last "Mark as read"
  unreviewed: number; // hunks changed outside mieruka, not yet accepted
  readAt?: number; // unix ms
}

export interface Doc extends DocMeta {
  summary?: string;
  blocks: Block[];
}

export interface Project {
  id: string;
  name: string;
  root: string;
}

export interface ContextSource {
  path: string;
  how: string; // why the agent loads it, or why not
  tokens: number | null; // null = not read
  onDemand: boolean; // loaded later, not at session start
  docId?: string;
}

export interface AgentContext {
  agent: string;
  label: string;
  projectId: string;
  dir: string;
  approximate: boolean;
  sources: ContextSource[];
}

export interface Workspace {
  projects: Project[];
  docs: DocMeta[];
  issues: Issue[];
  contexts: AgentContext[];
}

export const KIND_LABEL: Record<DocKind, string> = {
  instructions: "Instructions",
  plans: "Plans",
  notes: "Notes & memory",
};
