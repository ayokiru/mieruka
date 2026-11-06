import type { MouseEvent } from "react";
import type { Doc, Issue } from "../data/types";
import { formatReadAt } from "../data/api";

interface Props {
  doc: Doc;
  projectName: string;
  issues: Issue[];
  contextVisible: boolean;
  onToggleContext: () => void;
  onMarkRead: () => void;
  onAccept: (hunk: string) => void;
  onRevert: (hunk: string) => void;
  onOpenWiki: (name: string) => void;
  onIssue: (issueId: string) => void;
}

export function DocView(props: Props) {
  const { doc, issues, onAccept, onRevert } = props;
  const changed = doc.blocks.filter((b) => b.change === "changed").length;

  // Links and issue marks live inside rendered HTML, so handle clicks here.
  function onReaderClick(e: MouseEvent<HTMLElement>) {
    const el = e.target as HTMLElement;
    const wiki = el.closest<HTMLElement>("a[data-wikilink], [data-wiki]");
    if (wiki) {
      e.preventDefault();
      props.onOpenWiki(wiki.dataset.wiki ?? decodeURIComponent(wiki.getAttribute("href") ?? ""));
      return;
    }
    // Relative links point into the project; open them in mieruka instead of navigating away.
    const link = el.closest<HTMLAnchorElement>("a[href]");
    if (link && !/^[a-z]+:/i.test(link.getAttribute("href") ?? "")) {
      e.preventDefault();
      props.onOpenWiki((link.getAttribute("href") ?? "").split("#")[0]);
    }
    const mark = el.closest<HTMLElement>("mark[data-issue]");
    if (mark) props.onIssue(mark.dataset.issue!);
  }

  return (
    <>
      <header className="doc-bar">
        <div>
          {props.projectName} / <b>{doc.path}</b>
          <span className="sr-only">,</span> &nbsp;{doc.kind} · {doc.lines} lines
        </div>
        <div>
          <button
            className="btn icon"
            aria-pressed={props.contextVisible}
            aria-label="Agent panel"
            title={props.contextVisible ? "Hide agent panel (⌘⌥\\)" : "Show agent panel (⌘⌥\\)"}
            onClick={props.onToggleContext}
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
              <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="2" />
              <line x1="10" y1="3" x2="10" y2="13" />
            </svg>
          </button>
        </div>
      </header>

      <div className="doc-scroll">
        {changed > 0 && doc.readAt !== undefined && (
          <div className="notice" role="status">
            <span>
              <strong>{changed} {changed === 1 ? "change" : "changes"}</strong> since you read this {formatReadAt(doc.readAt)}
              <span className="hint"> · J / K to step through</span>
            </span>
            <span>
              <button className="btn" onClick={props.onMarkRead}>Mark as read</button>
            </span>
          </div>
        )}

        <article className="reader" onClick={onReaderClick}>
          {!doc.blocks[0]?.html.startsWith("<h1") && <h1>{doc.title}</h1>}
          {doc.summary && <div className="summary">{doc.summary}</div>}
          {doc.blocks.map((b, i) => (
            <section key={b.id} id={`block-${b.id}`} className={`block ${b.change ?? ""}`} tabIndex={b.change ? -1 : undefined}>
              {b.hunk && b.hunk !== doc.blocks[i - 1]?.hunk && (
                <div className="change-head">
                  <span>Unreviewed change{b.changedAt !== undefined && ` · ${formatReadAt(b.changedAt)}`}</span>
                  <span>
                    <button className="btn" onClick={() => onRevert(b.hunk!)}>Revert change</button>
                    <button className="btn primary" onClick={() => onAccept(b.hunk!)}>Accept change</button>
                  </span>
                </div>
              )}
              {b.change === "changed" && <span className="sr-only">Changed since you read it:</span>}
              {b.html && <div dangerouslySetInnerHTML={{ __html: b.html }} />}
              {b.previousHtml && (
                <div className="prev" aria-label="Before the change" dangerouslySetInnerHTML={{ __html: b.previousHtml }} />
              )}
              {issues
                .filter((i) => i.blockId === b.id)
                .map((i) => (
                  <div key={i.id} id={`tip-${i.id}`} className={`tip ${i.severity}`} role="note">
                    <strong>{i.title}</strong> · {i.message}
                  </div>
                ))}
            </section>
          ))}
        </article>
      </div>
    </>
  );
}
