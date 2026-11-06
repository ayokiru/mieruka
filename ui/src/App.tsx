import { useCallback, useEffect, useMemo, useState } from "react";
import type { Doc, Workspace } from "./data/types";
import { acceptHunk, loadDoc, loadWorkspace, markRead, revertHunk, undoRevert } from "./data/api";
import { Sidebar } from "./components/Sidebar";
import { DocView } from "./components/DocView";
import { ContextPanel } from "./components/ContextPanel";
import { Splitter } from "./components/Splitter";
import { CommandPalette } from "./components/CommandPalette";
import { Toast } from "./components/Toast";

const NARROW = 1200;
const REFRESH_MS = 5000;

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
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  el.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "center" });
  el.classList.remove("flash");
  void el.offsetWidth; // restart the animation
  el.classList.add("flash");
  if (el.tabIndex >= -1) el.focus({ preventScroll: true });
}

// Instruction files first, CLAUDE.md / AGENTS.md before the rest.
function firstDoc(ws: Workspace, projectId: string) {
  const docs = ws.docs.filter((d) => d.projectId === projectId);
  const rank = (p: string) => (/^(CLAUDE|AGENTS)\.md$/.test(p) ? 0 : 1);
  return [...docs].sort(
    (a, b) => Number(a.kind !== "instructions") - Number(b.kind !== "instructions") || rank(a.path) - rank(b.path),
  )[0];
}

export default function App() {
  const [ws, setWs] = useState<Workspace | null>(null);
  const [offline, setOffline] = useState(false);
  const [projectId, setProjectId] = useState("");
  const [docId, setDocId] = useState("");
  const [doc, setDoc] = useState<Doc | null>(null);
  const [agent, setAgent] = useState("claude");
  const [sideW, setSideW] = useState(248);
  const [ctxW, setCtxW] = useState(336);
  const [showSide, setShowSide] = useState(true);
  const narrow = useNarrow();
  const [ctxPref, setCtxPref] = useState<boolean | null>(null); // null = follow window width
  const showCtx = ctxPref ?? !narrow;
  const [palette, setPalette] = useState(false);
  const [toast, setToast] = useState<{ message: string; undo?: () => void } | null>(null);
  const [version, setVersion] = useState(0); // bumped after writes so the open document reloads
  const closeToast = useCallback(() => setToast(null), []);

  const refresh = useCallback(async () => {
    const result = await loadWorkspace();
    setWs(result.workspace);
    setOffline(result.offline);
  }, []);

  useEffect(() => {
    void refresh();
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") void refresh();
    }, REFRESH_MS);
    window.addEventListener("focus", refresh);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", refresh);
    };
  }, [refresh]);

  // The stored selection may point at something that disappeared; fall back to the first item.
  const pid = ws?.projects.some((p) => p.id === projectId) ? projectId : (ws?.projects[0]?.id ?? "");
  const currentId = ws?.docs.some((d) => d.id === docId) ? docId : ws ? (firstDoc(ws, pid)?.id ?? "") : "";
  const meta = ws?.docs.find((d) => d.id === currentId);

  // Reload the open document when it changes on disk or the selection changes.
  useEffect(() => {
    if (!currentId) return;
    let live = true;
    void loadDoc(currentId).then((d) => {
      // Offline edits (accept/revert on the mock) must survive refreshes.
      if (live) setDoc((prev) => (offline && prev?.id === d?.id ? prev : d));
    });
    return () => {
      live = false;
    };
  }, [currentId, meta?.lines, meta?.changes, meta?.unreviewed, meta?.readAt, offline, version]);
  const shown = doc && doc.id === currentId ? doc : null;

  const project = ws?.projects.find((p) => p.id === pid);
  const contexts = useMemo(() => ws?.contexts.filter((c) => c.projectId === pid) ?? [], [ws, pid]);
  const context = contexts.find((c) => c.agent === agent) ?? contexts[0];
  const docIssues = ws?.issues.filter((i) => i.docId === currentId) ?? [];
  const projectIssues = ws?.issues.filter((i) => ws.docs.find((d) => d.id === i.docId)?.projectId === pid) ?? [];

  const openDoc = useCallback(
    (id: string) => {
      const d = ws?.docs.find((x) => x.id === id);
      if (!d) return;
      setProjectId(d.projectId);
      setDocId(d.id);
      setPalette(false);
    },
    [ws],
  );

  function editBlocks(fn: (b: Doc["blocks"][number]) => Doc["blocks"][number]) {
    setDoc((d) => (d ? { ...d, blocks: d.blocks.map(fn) } : d));
  }

  async function write(action: () => Promise<void>) {
    try {
      await action();
    } catch (e) {
      setToast({ message: e instanceof Error ? e.message : String(e) });
    }
    await refresh();
    setVersion((v) => v + 1);
  }

  function accept(hunk: string) {
    if (!shown) return;
    if (offline) {
      editBlocks((b) => (b.hunk === hunk ? { ...b, change: undefined, hunk: undefined, previousHtml: undefined } : b));
      return;
    }
    void write(() => acceptHunk(shown.id, hunk));
  }

  function revert(hunk: string) {
    if (!shown) return;
    const id = shown.id;
    if (offline) {
      const before = shown;
      setToast({ message: "Change reverted", undo: () => setDoc(before) });
      // Offline the mock only knows the previous text of the last block in a hunk.
      setDoc({
        ...shown,
        blocks: shown.blocks.flatMap((b) =>
          b.hunk !== hunk ? [b] : b.previousHtml ? [{ id: b.id, html: b.previousHtml }] : [],
        ),
      });
      return;
    }
    void write(async () => {
      await revertHunk(id, hunk);
      setToast({ message: "Change reverted", undo: () => void write(() => undoRevert(id)) });
    });
  }

  function undoToast() {
    toast?.undo?.();
    setToast(null);
  }

  async function onMarkRead() {
    if (!shown) return;
    if (offline) {
      editBlocks((b) => (b.change === "changed" ? { ...b, change: undefined } : b));
      return;
    }
    await markRead(shown.id);
    await refresh();
  }

  function openWiki(name: string) {
    const target = ws?.docs.find(
      (d) =>
        d.projectId === pid && d.path.replace(/\.md$/i, "").split("/").pop()?.toLowerCase() === name.toLowerCase(),
    );
    if (target) openDoc(target.id);
  }

  function showIssue(issueId: string) {
    const issue = ws?.issues.find((i) => i.id === issueId);
    if (!issue) return;
    openDoc(issue.docId);
    setTimeout(() => flash(`tip-${issue.id}`), 150);
  }

  useEffect(() => {
    if (shown && project) document.title = `${shown.path} — ${project.name} · mieruka`;
  }, [shown, project]);

  // Global shortcuts: ⌘K search, ⌘\ sidebar, ⌘⌥\ agent panel, J/K step through changes, ⌘Z undo a revert.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && !e.shiftKey && e.key.toLowerCase() === "z" && toast?.undo && !isTyping(e)) {
        e.preventDefault();
        toast.undo();
        setToast(null);
      } else if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette(true);
      } else if (mod && e.code === "Backslash") {
        e.preventDefault();
        if (e.altKey) setCtxPref(!showCtx);
        else setShowSide((v) => !v);
      } else if (!mod && !isTyping(e) && shown && (e.key === "j" || e.key === "k")) {
        const ids = shown.blocks.filter((b) => b.change).map((b) => `block-${b.id}`);
        if (ids.length === 0) return;
        const current = ids.indexOf(document.activeElement?.id ?? "");
        const next = e.key === "j" ? (current + 1) % ids.length : (current - 1 + ids.length) % ids.length;
        flash(ids[current < 0 && e.key === "k" ? ids.length - 1 : next]);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown, showCtx, toast]);

  if (!ws) return <div className="empty">Loading workspace…</div>;

  return (
    <div className="app">
      <a className="skip-link" href="#doc">Skip to document</a>
      {showSide && project && (
        <>
          <div className="pane side" style={{ width: sideW }}>
            <Sidebar
              workspace={ws}
              projectId={pid}
              docId={currentId}
              offline={offline}
              onSelectProject={(id) => {
                setProjectId(id);
                setDocId(firstDoc(ws, id)?.id ?? "");
              }}
              onSelectDoc={openDoc}
              onOpenSearch={() => setPalette(true)}
            />
          </div>
          <Splitter label="Resize sidebar" width={sideW} min={200} max={400} direction={1} onWidth={setSideW} />
        </>
      )}

      <main className="main" id="doc" tabIndex={-1}>
        {shown && project ? (
          <DocView
            doc={shown}
            projectName={project.name}
            issues={docIssues}
            contextVisible={showCtx}
            onToggleContext={() => setCtxPref(!showCtx)}
            onMarkRead={onMarkRead}
            onAccept={accept}
            onRevert={revert}
            onOpenWiki={openWiki}
            onIssue={showIssue}
          />
        ) : (
          <div className="empty">No agent Markdown found in this project yet.</div>
        )}
      </main>

      {showCtx && (
        <>
          <Splitter label="Resize agent panel" width={ctxW} min={280} max={480} direction={-1} onWidth={setCtxW} />
          <div className="pane ctx" style={{ width: ctxW }}>
            {context ? (
              <ContextPanel
                contexts={contexts}
                context={context}
                issues={projectIssues}
                onAgent={setAgent}
                onOpenDoc={openDoc}
                onIssue={showIssue}
              />
            ) : (
              <div className="empty">Agents load nothing from here on their own. Open a project folder to see its context.</div>
            )}
          </div>
        </>
      )}

      {palette && <CommandPalette workspace={ws} onOpenDoc={openDoc} onClose={() => setPalette(false)} />}
      {toast && (
        <Toast
          message={toast.message}
          onUndo={toast.undo ? undoToast : undefined}
          onDone={closeToast}
        />
      )}
    </div>
  );
}
