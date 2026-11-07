import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Doc, Workspace } from "./data/types";
import { acceptHunk, loadDoc, loadWorkspace, markRead, revertHunk, undoRevert } from "./data/api";
import { Sidebar } from "./components/Sidebar";
import { DocView } from "./components/DocView";
import { ContextPanel } from "./components/ContextPanel";
import { Splitter } from "./components/Splitter";
import { CommandPalette } from "./components/CommandPalette";
import { Toast } from "./components/Toast";
import { drafts } from "./data/drafts";

// CodeMirror is only downloaded when someone starts editing.
const Editor = lazy(() => import("./components/Editor").then((m) => ({ default: m.Editor })));

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
  const rank = (p: string) => (p === "CLAUDE.md" ? 0 : p === "AGENTS.md" ? 1 : 2);
  return [...docs].sort(
    (a, b) => Number(a.kind !== "instructions") - Number(b.kind !== "instructions") || rank(a.path) - rank(b.path),
  )[0];
}

function resolvePath(from: string, href: string) {
  const parts = from.split("/").slice(0, -1);
  for (const seg of href.split("/")) {
    if (seg === "..") parts.pop();
    else if (seg && seg !== ".") parts.push(seg);
  }
  return parts.join("/");
}

const readHash = () => decodeURIComponent(window.location.hash.slice(1));

export default function App() {
  const [ws, setWs] = useState<Workspace | null>(null);
  const [offline, setOffline] = useState(false);
  const [projectId, setProjectId] = useState("");
  const [docId, setDocId] = useState("");
  // The address shows the open document as #project/path, so it can be linked and bookmarked.
  const [hash, setHash] = useState(readHash);
  const [doc, setDoc] = useState<Doc | null>(null);
  const [agent, setAgent] = useState("claude");
  const [sideW, setSideW] = useState(248);
  const [ctxW, setCtxW] = useState(336);
  const [showSide, setShowSide] = useState(true);
  const narrow = useNarrow();
  const [ctxPref, setCtxPref] = useState<boolean | null>(null); // null = follow window width
  const showCtx = ctxPref ?? !narrow;
  const [palette, setPalette] = useState(false);
  const [toast, setToast] = useState<{ message: string; undo?: () => void; error?: boolean } | null>(null);
  const [editingId, setEditingId] = useState("");
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
  const linkOf = useCallback(
    (d: { projectId: string; path: string }) => `${ws?.projects.find((p) => p.id === d.projectId)?.name}/${d.path}`,
    [ws],
  );
  const hashDoc = hash ? ws?.docs.find((d) => linkOf(d) === hash) : undefined;
  const pid = ws?.projects.some((p) => p.id === projectId)
    ? projectId
    : (hashDoc?.projectId ?? ws?.projects[0]?.id ?? "");
  const currentId = ws?.docs.some((d) => d.id === docId)
    ? docId
    : hashDoc && hashDoc.projectId === pid
      ? hashDoc.id
      : ws
        ? (firstDoc(ws, pid)?.id ?? "")
        : "";
  const meta = ws?.docs.find((d) => d.id === currentId);

  useEffect(() => {
    function onHash() {
      setHash(readHash());
      setProjectId("");
      setDocId("");
    }
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  const link = meta ? linkOf(meta) : "";
  useEffect(() => {
    if (link && readHash() !== link) history.replaceState(null, "", `#${encodeURI(link)}`);
  }, [link]);

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
      const text = e instanceof Error ? e.message : String(e);
      setToast({ message: text.charAt(0).toUpperCase() + text.slice(1), error: true });
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
      setToast({ message: "Change reverted", undo: () => undoLastRevert(id) });
    });
  }

  // Each call undoes one more revert; the core refuses if the file changed since.
  function undoLastRevert(id: string) {
    void write(async () => {
      await undoRevert(id);
      setToast({ message: "Revert undone" });
    });
  }
  const undoShortcut = useRef<() => boolean>(() => false);
  undoShortcut.current = () => {
    if (toast?.undo) {
      undoToast();
      return true;
    }
    if (shown && meta && meta.undoSteps > 0 && !offline && editingId !== shown.id) {
      undoLastRevert(shown.id);
      return true;
    }
    return false;
  };

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
    else setToast({ message: `No document named ${name} in this project` });
  }

  // Relative links and @imports resolve against the open document's folder, like Claude Code does.
  function openPath(href: string) {
    if (!meta) return;
    const target = href.startsWith("~/") ? href : resolvePath(meta.path, href);
    const hit =
      ws?.docs.find((d) => d.projectId === meta.projectId && d.path === target) ??
      ws?.docs.find((d) => d.path === target);
    if (hit) openDoc(hit.id);
    else setToast({ message: `${target} is not an agent doc in this workspace` });
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

  // Global shortcuts: ⌘K search, ⌘\ sidebar, ⌘⌥\ agent panel, E edit, J/K step through changes, ⌘Z undo a revert.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && !e.shiftKey && e.key.toLowerCase() === "z" && !isTyping(e)) {
        if (undoShortcut.current()) e.preventDefault();
      } else if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette(true);
      } else if (mod && e.code === "Backslash") {
        e.preventDefault();
        if (e.altKey) setCtxPref(!showCtx);
        else setShowSide((v) => !v);
      } else if (!mod && !e.altKey && !isTyping(e) && shown && !offline && e.key === "e" && editingId !== shown.id) {
        e.preventDefault();
        setEditingId(shown.id);
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
  }, [shown, showCtx, toast, offline, editingId]);

  // Warn before closing the tab with unsaved edits.
  useEffect(() => {
    function onUnload(e: BeforeUnloadEvent) {
      if (drafts.size > 0) e.preventDefault();
    }
    window.addEventListener("beforeunload", onUnload);
    return () => window.removeEventListener("beforeunload", onUnload);
  }, []);

  const closeEditor = useCallback(() => setEditingId(""), []);
  const onSaved = useCallback(
    (message: string) => {
      setToast({ message });
      void refresh();
      setVersion((v) => v + 1);
    },
    [refresh],
  );

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
        {meta && project && editingId === currentId ? (
          <Suspense fallback={<div className="empty">Loading editor…</div>}>
            <Editor key={meta.id} doc={meta} projectName={project.name} onClose={closeEditor} onSaved={onSaved} />
          </Suspense>
        ) : shown && project ? (
          <DocView
            doc={shown}
            projectName={project.name}
            issues={docIssues}
            contextVisible={showCtx}
            onToggleContext={() => setCtxPref(!showCtx)}
            onMarkRead={onMarkRead}
            onEdit={offline ? undefined : () => setEditingId(shown.id)}
            onAccept={accept}
            onRevert={revert}
            onOpenWiki={openWiki}
            onOpenPath={openPath}
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
          error={toast.error}
          onUndo={toast.undo ? undoToast : undefined}
          onDone={closeToast}
        />
      )}
    </div>
  );
}
