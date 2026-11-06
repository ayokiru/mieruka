// Shapes the Rust core will serve; the mock data uses the same ones.

export type DocKind = "instructions" | "plans" | "notes";
export type AgentId = "claude" | "codex" | "cursor";
export type Severity = "error" | "warning" | "info";

export interface Issue {
  id: string;
  rule: string; // e.g. "conflict", "stale-path"
  title: string; // short label shown in bold
  message: string; // what is wrong and how to fix it
  severity: Severity;
  docId: string;
  blockId: string;
  line: number;
}

export interface Block {
  id: string;
  // Rendered HTML for one block (heading, paragraph, list, code...).
  // Issue ranges are wrapped in <mark data-issue="id">.
  html: string;
  change?: "changed" | "unreviewed";
  // For unreviewed blocks: what the block looked like when last approved.
  previousHtml?: string;
  changedAt?: string;
}

export interface Doc {
  id: string;
  projectId: string;
  path: string;
  kind: DocKind;
  lines: number;
  title: string;
  summary?: string;
  blocks: Block[];
  readAt?: string; // when the user last marked it read
}

export interface Project {
  id: string;
  name: string;
  root: string;
}

export interface ContextSource {
  path: string;
  how: string; // why the agent loads it
  tokens: number | null; // null = not loaded
  docId?: string;
}

export interface AgentContext {
  agent: AgentId;
  dir: string;
  approximate: boolean;
  sources: ContextSource[];
}

export interface Workspace {
  projects: Project[];
  docs: Doc[];
  issues: Issue[];
  contexts: AgentContext[];
}

export const AGENTS: { id: AgentId; label: string }[] = [
  { id: "claude", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "cursor", label: "Cursor" },
];

export const KIND_LABEL: Record<DocKind, string> = {
  instructions: "Instructions",
  plans: "Plans",
  notes: "Notes & memory",
};
