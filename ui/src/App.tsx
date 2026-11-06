import { useCallback, useEffect, useMemo, useState } from "react";
import { mockWorkspace } from "./data/mock";
import type { AgentId, Workspace } from "./data/types";
import { Sidebar } from "./components/Sidebar";
import { DocView } from "./components/DocView";
import { ContextPanel } from "./components/ContextPanel";
import { Splitter } from "./components/Splitter";
import { CommandPalette } from "./components/CommandPalette";
import { Toast } from "./components/Toast";

const NARROW = 1200;

function useNarrow() {
  const [narrow, setNarrow] = useState(() => window.innerWidth < NARROW);
  useEffect(() => {
    const mq = window.matchMedia(`(max-width: ${NARROW - 1}px)`);
    const on = () => setNarrow(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return narrow;
}

function isTyping(e: KeyboardEvent) {
  const el = e.target as HTMLElement;
  return el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName);
}

function flash(id: string) {
  const el = document.getElementById(id);
  if (!el) return;
  el.scrollIntoView({ behavior: "smooth", block: "center" });
  el.classList.remove("flash");
  void el.offsetWidth; // restart the animation
  el.classList.add("flash");
  if (el.tabIndex >= -1) el.focus({ preventScroll: true });
}

export default function App() {
  const [ws, setWs] = useState<Workspace>(mockWorkspace);
  const [projectId, setProjectId] = useState("winrice");
  const [docId, setDocId] = useState("w-claude");
  const [agent, setAgent] = useState<AgentId>("claude");
  const [sideW, setSideW] = useState(248);
  const [ctxW, setCtxW] = useState(336);
  const [showSide, setShowSide] = useState(true);
  const narrow = useNarrow();
  const [ctxPref, setCtxPref] = useState<boolean | null>(null); // null = follow window width
  const showCtx = ctxPref ?? !narrow;
  const [palette, setPalette] = useState(false);
  const [toast, setToast] = useState<{ message: string; undo: Workspace } | null>(null);

  const closeToast = useCallback(() => setToast(null), []);
  const doc = ws.docs.find((d) => d.id === docId)!;
  const project = ws.projects.find((p) => p.id === projectId)!;
  const docIssues = ws.issues.filter((i) => i.docId === docId);
  const context = ws.contexts.find((c) => c.agent === agent)!;
  const unread = useMemo(() => new Set(ws.docs.filter((d) => d.blocks.some((b) => b.change)).map((d) => d.id)), [ws]);
  const projectIssues = ws.issues.filter((i) => ws.docs.find((d) => d.id === i.docId)?.projectId === projectId);

  const openDoc = useCallback(
    (id: string) => {
      const d = ws.docs.find((x) => x.id === id);
      if (!d) return;
      setProjectId(d.projectId);
      setDocId(d.id);
      setPalette(false);
    },
    [ws],
  );

  const updateBlocks = (fn: (b: Workspace["docs"][number]["blocks"][number]) => typeof b) =>
    setWs((w) => ({ ...w, docs: w.docs.map((d) => (d.id === docId ? { ...d, blocks: d.blocks.map(fn) } : d)) }));

  function accept(blockId: string) {
    updateBlocks((b) => (b.id === blockId ? { ...b, change: undefined, previousHtml: undefined } : b));
  }

  function revert(blockId: string) {
    setToast({ message: "Change reverted", undo: ws });
    updateBlocks((b) =>
      b.id === blockId ? { ...b, html: b.previousHtml ?? b.html, change: undefined, previousHtml: undefined } : b,
    );
  }

  function markRead() {
    setWs((w) => ({
      ...w,
      docs: w.docs.map((d) =>
        d.id === docId
          ? { ...d, readAt: "just now", blocks: d.blocks.map((b) => (b.change === "changed" ? { ...b, change: undefined } : b)) }
          : d,
      ),
    }));
  }

  function openWiki(name: string) {
    const target = ws.docs.find(
      (d) => d.projectId === projectId && d.path.replace(/\.md$/i, "").split("/").pop()?.toLowerCase() === name.toLowerCase(),
    );
    if (target) openDoc(target.id);
  }

  function showIssue(issueId: string) {
    const issue = ws.issues.find((i) => i.id === issueId);
    if (!issue) return;
    openDoc(issue.docId);
    requestAnimationFrame(() => flash(`tip-${issue.id}`));
  }

  // Global shortcuts: ⌘K search, ⌘\ sidebar, ⌘⌥\ agent panel, J/K step through changes.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette(true);
      } else if (mod && e.code === "Backslash") {
        e.preventDefault();
        if (e.altKey) setCtxPref(!showCtx);
        else setShowSide((v) => !v);
      } else if (!mod && !isTyping(e) && (e.key === "j" || e.key === "k")) {
        const ids = doc.blocks.filter((b) => b.change).map((b) => `block-${b.id}`);
        if (ids.length === 0) return;
        const current = ids.indexOf(document.activeElement?.id ?? "");
        const next = e.key === "j" ? (current + 1) % ids.length : (current - 1 + ids.length) % ids.length;
        flash(ids[current < 0 && e.key === "k" ? ids.length - 1 : next]);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [doc, showCtx]);

  return (
    <div className="app">
      {showSide && (
        <>
          <div className="pane side" style={{ width: sideW }}>
            <Sidebar
              workspace={ws}
              projectId={projectId}
              docId={docId}
              unread={unread}
              onSelectProject={(id) => {
                setProjectId(id);
                const first = ws.docs.find((d) => d.projectId === id);
                if (first) setDocId(first.id);
              }}
              onSelectDoc={openDoc}
              onOpenSearch={() => setPalette(true)}
            />
          </div>
          <Splitter label="Resize sidebar" width={sideW} min={200} max={400} direction={1} onWidth={setSideW} />
        </>
      )}

      <main className="main">
        {doc ? (
          <DocView
            doc={doc}
            projectName={project.name}
            issues={docIssues}
            contextVisible={showCtx}
            onToggleContext={() => setCtxPref(!showCtx)}
            onMarkRead={markRead}
            onAccept={accept}
            onRevert={revert}
            onOpenWiki={openWiki}
            onIssue={showIssue}
          />
        ) : (
          <div className="empty">This project has no agent docs yet.</div>
        )}
      </main>

      {showCtx && context && (
        <>
          <Splitter label="Resize agent panel" width={ctxW} min={280} max={480} direction={-1} onWidth={setCtxW} />
          <div className="pane ctx" style={{ width: ctxW }}>
            <ContextPanel
              context={context}
              agent={agent}
              issues={projectIssues}
              onAgent={setAgent}
              onOpenDoc={openDoc}
              onIssue={showIssue}
            />
          </div>
        </>
      )}

      {palette && <CommandPalette workspace={ws} onOpenDoc={openDoc} onClose={() => setPalette(false)} />}
      {toast && (
        <Toast
          message={toast.message}
          onUndo={() => {
            setWs(toast.undo);
            setToast(null);
          }}
          onDone={closeToast}
        />
      )}
    </div>
  );
}
