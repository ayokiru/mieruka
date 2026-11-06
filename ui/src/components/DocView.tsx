import type { MouseEvent } from "react";
import type { Doc, Issue } from "../data/types";

interface Props {
  doc: Doc;
  projectName: string;
  issues: Issue[];
  contextVisible: boolean;
  onToggleContext: () => void;
  onMarkRead: () => void;
  onAccept: (blockId: string) => void;
  onRevert: (blockId: string) => void;
  onOpenWiki: (name: string) => void;
  onIssue: (issueId: string) => void;
}

export function DocView(props: Props) {
  const { doc, issues, onAccept, onRevert } = props;
  const changed = doc.blocks.filter((b) => b.change).length;

  // Links and issue marks live inside rendered HTML, so handle clicks here.
  function onReaderClick(e: MouseEvent<HTMLElement>) {
    const el = e.target as HTMLElement;
    const wiki = el.closest<HTMLElement>("[data-wiki]");
    if (wiki) {
      e.preventDefault();
      props.onOpenWiki(wiki.dataset.wiki!);
      return;
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
          <button className="btn">Edit</button>
          <button
            className="btn icon"
            aria-pressed={props.contextVisible}
            title={props.contextVisible ? "Hide agent panel (⌘⌥\\)" : "Show agent panel (⌘⌥\\)"}
            onClick={props.onToggleContext}
          >
            <span aria-hidden="true">⇥</span>
            <span className="sr-only">Agent panel</span>
          </button>
        </div>
      </header>

      <div className="doc-scroll">
        {changed > 0 && doc.readAt && (
          <div className="notice" role="status">
            <span>
              <strong>{changed} {changed === 1 ? "change" : "changes"}</strong> since you read this {doc.readAt}
              <span className="hint"> · J / K to step through</span>
            </span>
            <span>
              <button className="btn primary" onClick={props.onMarkRead}>Mark as read</button>
            </span>
          </div>
        )}

        <article className="reader" onClick={onReaderClick}>
          <h1>{doc.title}</h1>
          {doc.summary && <div className="summary">{doc.summary}</div>}
          {doc.blocks.map((b) => (
            <section key={b.id} id={`block-${b.id}`} className={`block ${b.change ?? ""}`} tabIndex={b.change ? -1 : undefined}>
              {b.change === "unreviewed" && (
                <div className="change-head">
                  <span>Unreviewed change · {b.changedAt}</span>
                  <span>
                    <button className="btn" onClick={() => onRevert(b.id)}>Revert change</button>
                    <button className="btn primary" onClick={() => onAccept(b.id)}>Accept change</button>
                  </span>
                </div>
              )}
              {b.change === "changed" && <span className="sr-only">Changed since you read it:</span>}
              <div dangerouslySetInnerHTML={{ __html: b.html }} />
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
