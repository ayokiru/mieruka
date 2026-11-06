import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type { Workspace } from "../data/types";

interface Props {
  workspace: Workspace;
  onOpenDoc: (docId: string) => void;
  onClose: () => void;
}

export function CommandPalette({ workspace, onOpenDoc, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  // Return focus to whatever was focused before the palette opened.
  useEffect(() => {
    const before = document.activeElement as HTMLElement | null;
    input.current?.focus();
    return () => before?.focus();
  }, []);

  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    return workspace.docs
      .map((d) => ({ doc: d, project: workspace.projects.find((p) => p.id === d.projectId)!.name }))
      .filter(({ doc, project }) => !q || `${project} ${doc.path} ${doc.title}`.toLowerCase().includes(q))
      .slice(0, 20);
  }, [query, workspace]);

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
    else if (e.key === "ArrowDown") setIndex((i) => Math.min(results.length - 1, i + 1));
    else if (e.key === "ArrowUp") setIndex((i) => Math.max(0, i - 1));
    else if (e.key === "Enter" && results[index]) onOpenDoc(results[index].doc.id);
    else if (e.key === "Tab") e.preventDefault(); // keep focus inside the dialog
    else return;
    e.preventDefault();
  }

  return (
    <div className="overlay" onMouseDown={onClose}>
      <div
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label="Search agent docs"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={onKeyDown}
      >
        <input
          ref={input}
          value={query}
          placeholder="Search agent docs"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-results"
          aria-activedescendant={results[index] ? `res-${results[index].doc.id}` : undefined}
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(0);
          }}
        />
        {results.length === 0 ? (
          <div className="empty">No documents match “{query}”.</div>
        ) : (
          <ul id="palette-results" role="listbox">
            {results.map(({ doc, project }, i) => (
              <li
                key={doc.id}
                id={`res-${doc.id}`}
                role="option"
                aria-selected={i === index}
                onMouseEnter={() => setIndex(i)}
                onClick={() => onOpenDoc(doc.id)}
              >
                <span className="mono">{doc.path}</span>
                <span className="hint">{project}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
