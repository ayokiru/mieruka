import { useEffect, useRef, useState } from "react";
import { EditorView, keymap, drawSelection, highlightActiveLine } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import { tags as t } from "@lezer/highlight";
import type { DocMeta } from "../data/types";
import { loadSource, renderPreview, saveSource } from "../data/api";
import { drafts } from "../data/drafts";


const highlight = HighlightStyle.define([
  { tag: t.heading, fontWeight: "600", color: "var(--text)" },
  { tag: [t.emphasis], fontStyle: "italic" },
  { tag: [t.strong], fontWeight: "600" },
  { tag: [t.link, t.url], color: "var(--indigo)" },
  // Gold is reserved for changes and red for issues, so code stays neutral here.
  { tag: [t.monospace], color: "var(--text)", backgroundColor: "var(--raised)", borderRadius: "4px" },
  { tag: [t.processingInstruction, t.meta, t.contentSeparator, t.list, t.quote], color: "var(--faint)" },
  { tag: t.comment, color: "var(--faint)", fontStyle: "italic" },
]);

const theme = EditorView.theme({
  "&": { height: "100%", fontSize: "13.5px", background: "transparent", color: "var(--text)" },
  ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.7", padding: "16px 0 48px" },
  ".cm-content": { padding: "0 28px", caretColor: "var(--text)" },
  ".cm-cursor": { borderLeftColor: "var(--text)" },
  "&.cm-focused": { outline: "none" },
  ".cm-activeLine": { background: "var(--indigo-bg)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": { background: "var(--indigo-bg) !important" },
  ".cm-selectionMatch": { outline: "1px solid var(--border)", borderRadius: "2px" },
  ".cm-panels": { background: "var(--panel)", color: "var(--text)", borderColor: "var(--border)" },
});

interface Props {
  doc: DocMeta;
  projectName: string;
  onClose: () => void;
  onSaved: (message: string) => void;
}

export function Editor({ doc, projectName, onClose, onSaved }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const [draft, setDraft] = useState(() => drafts.get(doc.id) ?? null);
  const [original, setOriginal] = useState<string | null>(null);
  const [html, setHtml] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const dirty = draft !== null && original !== null && draft.text !== original;

  // Load the file once; a stored draft wins over the disk text.
  useEffect(() => {
    let live = true;
    loadSource(doc.id)
      .then((s) => {
        if (!live) return;
        setOriginal(s.text);
        setDraft((d) => d ?? s);
      })
      .catch((e: Error) => live && setError(e.message));
    return () => {
      live = false;
    };
  }, [doc.id]);

  // Create CodeMirror once the text is known.
  const ready = draft !== null;
  useEffect(() => {
    if (!ready || !host.current || view.current) return;
    const start = drafts.get(doc.id)?.text ?? draft!.text;
    view.current = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: start,
        extensions: [
          history(),
          drawSelection(),
          highlightActiveLine(),
          highlightSelectionMatches(),
          EditorView.lineWrapping,
          markdown(),
          syntaxHighlighting(highlight),
          theme,
          keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
          EditorView.contentAttributes.of({ "aria-label": `Markdown source of ${doc.path}` }),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) setDraft((d) => (d ? { ...d, text: u.state.doc.toString() } : d));
          }),
        ],
      }),
    });
    view.current.focus();
    return () => {
      view.current?.destroy();
      view.current = null;
    };
    // The editor owns its text after creation; later draft updates must not recreate it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, doc.id]);

  // Keep the draft across document switches; drop it once it matches the file again.
  useEffect(() => {
    if (original === null) return;
    if (draft && draft.text !== original) drafts.set(doc.id, draft);
    else drafts.delete(doc.id);
  }, [draft, original, doc.id]);

  // Live preview, rendered by the core so it matches the reader exactly.
  const text = draft?.text;
  useEffect(() => {
    if (text === undefined) return;
    const timer = setTimeout(() => {
      renderPreview(text).then(setHtml, () => {});
    }, 200);
    return () => clearTimeout(timer);
  }, [text]);

  async function save() {
    if (!draft || !dirty || saving) return;
    setSaving(true);
    setError("");
    try {
      const saved = await saveSource(doc.id, draft.text, draft.base);
      drafts.delete(doc.id);
      setOriginal(saved.text);
      setDraft(saved);
      onSaved(doc.unreviewed > 0 ? "Saved · unreviewed changes accepted" : "Saved");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  function discard() {
    drafts.delete(doc.id);
    onClose();
  }

  // ⌘S saves, Esc leaves when there is nothing to lose.
  const saveRef = useRef(save);
  saveRef.current = save;
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void saveRef.current();
      } else if (e.key === "Escape" && !dirty && !document.querySelector(".cm-panels, [role=dialog]")) {
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dirty, onClose]);

  return (
    <>
      <header className="doc-bar">
        <div>
          {projectName} / <b>{doc.path}</b>
          <span className="sr-only">,</span> &nbsp;
          <span className={dirty ? "edit-state dirty" : "edit-state"}>{dirty ? "Edited" : "Editing"}</span>
        </div>
        <div>
          {dirty ? (
            <button className="btn" onClick={discard}>Discard changes</button>
          ) : (
            <button className="btn" onClick={onClose} title="Esc">Done</button>
          )}
          <button className="btn primary" onClick={() => void save()} disabled={!dirty || saving} title="⌘S">
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </header>
      {(error || doc.unreviewed > 0) && (
        <div className={`notice ${error ? "error" : ""}`} role={error ? "alert" : "status"}>
          <span>
            {error ||
              `${doc.unreviewed} unreviewed ${doc.unreviewed === 1 ? "change" : "changes"} in this file. Saving accepts the whole file as reviewed.`}
          </span>
        </div>
      )}
      <div className="edit-split">
        <div className="edit-source" ref={host}>
          {!ready && !error && <div className="empty">Loading source…</div>}
        </div>
        <div className="edit-preview doc-scroll" aria-label="Preview">
          <article className="reader" dangerouslySetInnerHTML={{ __html: html }} />
        </div>
      </div>
    </>
  );
}
