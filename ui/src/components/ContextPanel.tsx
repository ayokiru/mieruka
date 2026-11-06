import type { KeyboardEvent } from "react";
import { AGENTS, type AgentContext, type AgentId, type Issue } from "../data/types";

interface Props {
  context: AgentContext;
  agent: AgentId;
  issues: Issue[];
  onAgent: (id: AgentId) => void;
  onOpenDoc: (docId: string) => void;
  onIssue: (issueId: string) => void;
}

function fmt(n: number) {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n);
}

export function ContextPanel({ context, agent, issues, onAgent, onOpenDoc, onIssue }: Props) {
  const loaded = context.sources.filter((s) => s.tokens !== null);
  const total = loaded.reduce((sum, s) => sum + (s.tokens ?? 0), 0);
  const max = Math.max(...loaded.map((s) => s.tokens ?? 0), 1);
  const approx = context.approximate ? "≈" : "";

  // Arrow keys move between agents, as in a native segmented control.
  function onKey(e: KeyboardEvent<HTMLDivElement>) {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    const i = AGENTS.findIndex((a) => a.id === agent);
    const next = AGENTS[(i + (e.key === "ArrowRight" ? 1 : AGENTS.length - 1)) % AGENTS.length];
    onAgent(next.id);
    e.currentTarget.querySelector<HTMLButtonElement>(`[data-agent="${next.id}"]`)?.focus();
  }

  return (
    <aside aria-label="What the agent sees">
      <h2>What the agent sees</h2>
      <div className="sub">in <span className="mono">{context.dir}</span></div>

      <div className="segmented" role="radiogroup" aria-label="Agent" onKeyDown={onKey}>
        {AGENTS.map((a) => (
          <button
            key={a.id}
            role="radio"
            data-agent={a.id}
            aria-checked={a.id === agent}
            tabIndex={a.id === agent ? 0 : -1}
            onClick={() => onAgent(a.id)}
          >
            {a.label}
          </button>
        ))}
      </div>

      <div className="total">
        <b>{approx}{fmt(total)}</b>
        <span>tokens loaded at start</span>
      </div>
      {context.approximate && (
        <div className="note">
          {agent === "claude" ? "Estimated: Anthropic doesn't publish Claude's tokenizer." : "Estimated token counts."}
        </div>
      )}

      {context.sources.map((s) => {
        const skipped = s.tokens === null;
        return (
          <button
            key={s.path}
            className={`source ${skipped ? "skipped" : ""}`}
            disabled={!s.docId}
            onClick={() => s.docId && onOpenDoc(s.docId)}
            aria-label={`${s.path}, ${s.how}${skipped ? "" : `, ${approx}${fmt(s.tokens!)} tokens`}`}
          >
            <span className="row">
              <span className="path">{s.path}</span>
              <span className="tokens">{skipped ? "—" : fmt(s.tokens!)}</span>
            </span>
            <span className="how">{s.how}</span>
            {!skipped && (
              <span className="meter" aria-hidden="true">
                <i style={{ width: `${Math.max(4, (s.tokens! / max) * 100)}%` }} />
              </span>
            )}
          </button>
        );
      })}

      {issues.length > 0 && (
        <div className="issues">
          <h2>Issues</h2>
          {issues.map((i) => (
            <button key={i.id} className={`issue ${i.severity}`} onClick={() => onIssue(i.id)}>
              <strong>{i.title}</strong> {i.message}
            </button>
          ))}
        </div>
      )}
    </aside>
  );
}
