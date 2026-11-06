import type { KeyboardEvent } from "react";
import { KIND_LABEL, type DocKind, type Workspace } from "../data/types";

interface Props {
  workspace: Workspace;
  projectId: string;
  docId: string;
  offline: boolean;
  onSelectProject: (id: string) => void;
  onSelectDoc: (id: string) => void;
  onOpenSearch: () => void;
}

const KINDS: DocKind[] = ["instructions", "plans", "notes"];

// Up/Down move focus between sidebar items, like a native list.
function moveFocus(e: KeyboardEvent<HTMLElement>) {
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  const items = [...e.currentTarget.querySelectorAll<HTMLButtonElement>(".nav-item")];
  const i = items.indexOf(document.activeElement as HTMLButtonElement);
  if (i < 0) return;
  e.preventDefault();
  items[Math.max(0, Math.min(items.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))].focus();
}

export function Sidebar({ workspace, projectId, docId, offline, onSelectProject, onSelectDoc, onOpenSearch }: Props) {
  const project = workspace.projects.find((p) => p.id === projectId)!;
  const docs = workspace.docs.filter((d) => d.projectId === projectId);
  const issueCount = workspace.issues.filter((i) => docs.some((d) => d.id === i.docId)).length;
  const others = workspace.projects.filter((p) => p.id !== projectId);

  return (
    <nav aria-label="Projects and documents" onKeyDown={moveFocus}>
      <div className="brand"><i aria-hidden="true" />mieruka</div>
      {offline && <div className="offline" role="status">Demo data: run the mieruka binary to see your files.</div>}
      <button className="search" onClick={onOpenSearch}>
        <span>Search all agent docs</span>
        <kbd className="mono">⌘K</kbd>
      </button>

      <div className="project-head">
        <h2>{project.name}</h2>
        {issueCount > 0 && <span className="count-issues">{issueCount} issues</span>}
      </div>

      {KINDS.map((kind) => {
        const list = docs.filter((d) => d.kind === kind);
        if (list.length === 0) return null;
        return (
          <div key={kind} role="group" aria-label={KIND_LABEL[kind]}>
            <div className="group-title" aria-hidden="true">{KIND_LABEL[kind]}</div>
            {list.map((d) => (
              <button
                key={d.id}
                className="nav-item"
                aria-current={d.id === docId ? "page" : undefined}
                title={d.path}
                onClick={() => onSelectDoc(d.id)}
              >
                <span className="name">{d.path}</span>
                {(d.unreviewed > 0 || d.changes > 0) && (
                  <span className="dot" title={d.unreviewed > 0 ? "Unreviewed changes" : "Changed since you read it"}>
                    <span className="sr-only">{d.unreviewed > 0 ? "Unreviewed changes" : "Changed since you read it"}</span>
                  </span>
                )}
              </button>
            ))}
          </div>
        );
      })}

      {others.length > 0 && (
        <>
          <div className="group-title">Other projects</div>
          {others.map((p) => (
            <button key={p.id} className="nav-item project" onClick={() => onSelectProject(p.id)}>
              <span className="name">{p.name}</span>
            </button>
          ))}
        </>
      )}
    </nav>
  );
}
