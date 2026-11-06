import type { AgentContext, Doc, DocMeta, Issue, Project, Workspace } from "./types";

// Stand-in used when the UI runs without the Rust core.
const data: { projects: Project[]; docs: (Omit<Doc, "changes" | "unreviewed" | "readAt"> & { readAt?: string })[]; issues: Issue[]; contexts: Omit<AgentContext, "projectId" | "label">[] } = {
  projects: [
    { id: "winrice", name: "winrice", root: "~/Projects/winrice" },
    { id: "pakwatch", name: "pakwatch", root: "~/Projects/pakwatch" },
    { id: "modrelease", name: "modrelease", root: "~/Projects/modrelease" },
  ],
  docs: [
    {
      id: "w-claude",
      projectId: "winrice",
      path: "CLAUDE.md",
      kind: "instructions",
      lines: 142,
      title: "winrice",
      summary: "Rust CLI: capture, diff, apply and rollback of the whole Windows look.",
      readAt: "1",
      blocks: [
        { id: "b1", html: "<h2>Rules</h2>" },
        { id: "b2", html: "<ul><li>Code comments: English only, few and basic.</li></ul>" },
        {
          id: "b3",
          change: "changed",
          html: '<ul><li><mark data-issue="i1">Install dependencies with <code>npm install</code> before running scripts.</mark></li></ul>',
        },
        {
          id: "b4",
          change: "unreviewed",
          hunk: "h1",
          changedAt: Date.now() - 2 * 3_600_000,
          previousHtml: "<ul><li>Commit at any time.</li></ul>",
          html: "<ul><li>Commit with <code>TZ=UTC</code> so no timezone offset leaks.</li></ul>",
        },
        { id: "b5", html: "<h2>Dev</h2>" },
        {
          id: "b6",
          html: "<ul><li><code>cargo test</code> runs everywhere: modules talk to the OS only through <code>sys::System</code>.</li></ul>",
        },
        {
          id: "b7",
          html: '<ul><li><mark data-issue="i2">Run <code>scripts/build-win.ps1</code> for local Windows builds.</mark></li></ul>',
        },
        {
          id: "b8",
          html: '<p>See <a href="#" data-wiki="TESTING">[[TESTING]]</a> before every release.</p>',
        },
      ],
    },
    {
      id: "w-agents",
      projectId: "winrice",
      path: "AGENTS.md",
      kind: "instructions",
      lines: 38,
      title: "Agent notes",
      blocks: [
        { id: "a1", html: "<h2>Tooling</h2>" },
        { id: "a2", html: "<ul><li>Use <code>pnpm</code> for every JavaScript task.</li></ul>" },
      ],
    },
    {
      id: "w-src",
      projectId: "winrice",
      path: "src/CLAUDE.md",
      kind: "instructions",
      lines: 12,
      title: "src",
      blocks: [{ id: "s1", html: "<p>Modules never call Windows APIs directly.</p>" }],
    },
    {
      id: "w-plan",
      projectId: "winrice",
      path: "plans/rollback-unset-values.md",
      kind: "plans",
      lines: 64,
      title: "Rollback for values that did not exist",
      blocks: [
        { id: "p1", html: "<h2>Goal</h2>" },
        { id: "p2", html: "<p>Rollback must delete registry values that were missing before an apply.</p>" },
      ],
    },
    {
      id: "w-accent",
      projectId: "winrice",
      path: "plans/accent-palette.md",
      kind: "plans",
      lines: 41,
      title: "Accent palette",
      blocks: [{ id: "c1", html: "<p>Captured palettes are applied as is.</p>" }],
    },
    {
      id: "w-testing",
      projectId: "winrice",
      path: "TESTING.md",
      kind: "notes",
      lines: 88,
      title: "Windows test plan",
      readAt: "2",
      blocks: [
        { id: "t1", html: "<h2>Capture</h2>" },
        { id: "t2", change: "changed", html: "<p>Run <code>winrice capture test</code> and check <code>setup.toml</code>.</p>" },
      ],
    },
    {
      id: "w-memory",
      projectId: "winrice",
      path: "memory/anonymity.md",
      kind: "notes",
      lines: 9,
      title: "Anonymity",
      blocks: [{ id: "m1", html: "<p>Commit as ayokiru with UTC dates.</p>" }],
    },
  ],
  issues: [
    {
      id: "i1",
      rule: "conflict",
      title: "Conflict",
      message: "AGENTS.md:12 says “use pnpm”. Claude Code loads both files here.",
      severity: "error",
      docId: "w-claude",
      blockId: "b3",
      line: 9,
    },
    {
      id: "i2",
      rule: "stale-path",
      title: "Stale path",
      message: "scripts/build-win.ps1 does not exist. Did you mean scripts/build-win.sh?",
      severity: "warning",
      docId: "w-claude",
      blockId: "b7",
      line: 31,
    },
  ],
  contexts: [
    {
      agent: "claude",
      dir: "winrice/src",
      approximate: true,
      sources: [
        { path: "~/.claude/CLAUDE.md", how: "user · always", tokens: 1200, onDemand: false },
        { path: "winrice/CLAUDE.md", how: "project · always", tokens: 3400, docId: "w-claude", onDemand: false },
        { path: "↳ @AGENTS.md", how: "imported on line 3", tokens: 800, docId: "w-agents", onDemand: false },
        { path: "src/CLAUDE.md", how: "nested · when files in src/ are read", tokens: 400, docId: "w-src", onDemand: true },
        { path: "8 skills", how: "descriptions only", tokens: 600, onDemand: false },
        { path: ".cursor/rules/rust.mdc", how: "not read by Claude Code", tokens: null, onDemand: false },
      ],
    },
    {
      agent: "codex",
      dir: "winrice/src",
      approximate: false,
      sources: [
        { path: "~/.codex/AGENTS.md", how: "user · always", tokens: 700, onDemand: false },
        { path: "winrice/AGENTS.md", how: "project · always", tokens: 790, docId: "w-agents", onDemand: false },
        { path: "winrice/CLAUDE.md", how: "not read by Codex", tokens: null, docId: "w-claude", onDemand: false },
      ],
    },
    {
      agent: "cursor",
      dir: "winrice/src",
      approximate: false,
      sources: [
        { path: ".cursor/rules/rust.mdc", how: "glob src/**/*.rs", tokens: 520, onDemand: false },
        { path: "winrice/AGENTS.md", how: "project · always", tokens: 790, docId: "w-agents", onDemand: false },
        { path: "winrice/CLAUDE.md", how: "symlinked to AGENTS.md · counted twice", tokens: 790, docId: "w-claude", onDemand: false },
      ],
    },
  ],
};

const LABELS: Record<string, string> = { claude: "Claude Code", codex: "Codex", cursor: "Cursor" };
const DAY = 86_400_000;

export const mockDocs: Record<string, Doc> = Object.fromEntries(
  data.docs.map((d) => {
    const changes = d.blocks.filter((b) => b.change).length;
    const unreviewed = new Set(d.blocks.map((b) => b.hunk).filter(Boolean)).size;
    const readAt = d.readAt ? Date.now() - Number(d.readAt) * DAY : undefined;
    return [d.id, { ...d, changes, unreviewed, readAt }];
  }),
);

export const mockWorkspace: Workspace = {
  projects: data.projects,
  docs: Object.values(mockDocs).map(({ blocks: _b, summary: _s, ...meta }) => meta as DocMeta),
  issues: data.issues,
  contexts: data.contexts.map((c) => ({ ...c, projectId: "winrice", label: LABELS[c.agent] })),
};
