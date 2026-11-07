//! Builds the whole workspace (projects, docs, contexts, issues) from disk.

use crate::checks::{self, CheckedDoc};
use crate::model::*;
use crate::{context, discover, markdown, reads, review, util};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Entry {
    pub meta: DocMeta,
    pub path: PathBuf,
    pub text: String,
    pub parsed: markdown::Parsed,
    /// Instructions and memory are tracked for unreviewed changes.
    pub review: Option<review::Review>,
    pub modified: Option<u64>,
}

pub struct Snapshot {
    pub workspace: Workspace,
    pub docs: HashMap<String, Entry>,
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn modified(path: &Path) -> Option<u64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64)
}

fn entry(
    path: PathBuf,
    kind: DocKind,
    tracked: bool,
    project_id: &str,
    base: Option<&Path>,
) -> Option<Entry> {
    let text = util::read(&path)?;
    let parsed = markdown::parse(&text);
    let id = util::id_for(&path);
    let review = tracked.then(|| review::load(&id, &text, &parsed)).flatten();
    let sources: Vec<&str> = parsed.blocks.iter().map(|b| b.source.as_str()).collect();
    let state = reads::state(&id, &sources);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let meta = DocMeta {
        id,
        project_id: project_id.to_string(),
        path: util::display(&path, base),
        kind,
        lines: text.lines().count(),
        title: parsed.title.clone().unwrap_or(name),
        changes: state.changed.len(),
        unreviewed: review.as_ref().map_or(0, |r| r.hunks.len()),
        read_at: state.read_at,
    };
    Some(Entry {
        meta,
        modified: modified(&path),
        path,
        text,
        parsed,
        review,
    })
}

/// `interactive` is the UI: it tracks files for review (writes ~/.mieruka) and lists the agents' home folders.
/// The CLI builds read-only and per project.
pub fn build(roots: &[PathBuf], interactive: bool) -> Snapshot {
    let mut projects = Vec::new();
    let mut docs: HashMap<String, Entry> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut issues = Vec::new();
    let mut contexts = Vec::new();

    for root in roots {
        let pid = util::id_for(root);
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string());
        projects.push(Project {
            id: pid.clone(),
            name,
            root: util::display(root, None),
        });

        let mut found: Vec<(discover::Found, bool)> = discover::project_docs(root)
            .into_iter()
            .map(|f| {
                let tracked = interactive && f.kind == DocKind::Instructions;
                (f, tracked)
            })
            .collect();
        found.extend(
            discover::memory_docs(root)
                .into_iter()
                .map(|f| (f, interactive)),
        );
        let mut ids = Vec::new();
        for (f, tracked) in found {
            let base = if f.path.starts_with(root) {
                Some(root.as_path())
            } else {
                None
            };
            if let Some(e) = entry(f.path, f.kind, tracked, &pid, base) {
                ids.push(e.meta.id.clone());
                order.push(e.meta.id.clone());
                docs.insert(e.meta.id.clone(), e);
            }
        }

        let doc_ids: HashMap<PathBuf, String> = ids
            .iter()
            .filter_map(|id| Some((crate::util::canonical(&docs[id].path).ok()?, id.clone())))
            .collect();
        let instruction_files: Vec<PathBuf> = ids
            .iter()
            .filter(|id| docs[*id].meta.kind == DocKind::Instructions)
            .map(|id| docs[id].path.clone())
            .collect();
        let claude = context::claude(root, &instruction_files, &doc_ids);
        let codex = context::codex(root, &instruction_files, &doc_ids);

        let checked: Vec<CheckedDoc> = ids
            .iter()
            .map(|id| {
                let e = &docs[id];
                CheckedDoc {
                    id,
                    path: &e.path,
                    parsed: &e.parsed,
                    lines: e.meta.lines,
                    instruction: e.meta.kind == DocKind::Instructions,
                }
            })
            .collect();
        issues.extend(checks::run(root, &checked, &claude, &codex));

        let dir = util::display(root, root.parent());
        for (agent, label, built, approximate) in [
            ("claude", "Claude Code", claude, true),
            ("codex", "Codex", codex, true),
        ] {
            contexts.push(AgentContext {
                agent,
                label,
                project_id: pid.clone(),
                dir: dir.clone(),
                approximate,
                sources: built.sources,
            });
        }
    }

    // Agents' home folders as their own project.
    let global = if interactive {
        discover::global_docs()
    } else {
        Vec::new()
    };
    if !global.is_empty() {
        projects.push(Project {
            id: "home".into(),
            name: "Home".into(),
            root: "~".into(),
        });
        for f in global {
            let tracked = f.kind == DocKind::Instructions;
            if let Some(e) = entry(f.path, f.kind, tracked, "home", None) {
                order.push(e.meta.id.clone());
                docs.entry(e.meta.id.clone()).or_insert(e);
            }
        }
    }

    let metas = order
        .iter()
        .filter_map(|id| docs.get(id).map(|e| e.meta.clone()))
        .collect();
    Snapshot {
        workspace: Workspace {
            projects,
            docs: metas,
            issues,
            contexts,
        },
        docs,
    }
}

/// Full document with change marks and issue underlines.
pub fn doc(snap: &Snapshot, id: &str) -> Option<Doc> {
    let e = snap.docs.get(id)?;
    let sources: Vec<&str> = e.parsed.blocks.iter().map(|b| b.source.as_str()).collect();
    let state = reads::state(id, &sources);
    let issues: Vec<&Issue> = snap
        .workspace
        .issues
        .iter()
        .filter(|i| i.doc_id == id)
        .collect();
    let mut blocks: Vec<Block> = e
        .parsed
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let block_id = format!("b{i}");
            let mut html = b.html.clone();
            for issue in issues.iter().filter(|x| x.block_id == block_id) {
                let Some(needle) = &issue.needle else {
                    continue;
                };
                let code = format!("<code>{}</code>", escape(needle));
                let target = if html.contains(&code) {
                    code
                } else {
                    escape(needle)
                };
                if let Some(pos) = html.find(&target) {
                    html.replace_range(
                        pos..pos + target.len(),
                        &format!("<mark data-issue=\"{}\">{target}</mark>", issue.id),
                    );
                }
            }
            let change =
                (state.read_at.is_some() && state.changed.contains(&i)).then_some("changed");
            Block {
                id: block_id,
                html,
                change,
                hunk: None,
                previous_html: None,
                changed_at: None,
            }
        })
        .collect();
    if let Some(r) = &e.review {
        mark_unreviewed(&mut blocks, r, e.modified);
    }
    Some(Doc {
        meta: e.meta.clone(),
        blocks,
    })
}

/// Mark hunk blocks as unreviewed; a pure deletion gets an empty block that only shows the old text.
fn mark_unreviewed(blocks: &mut Vec<Block>, r: &review::Review, modified: Option<u64>) {
    for h in r.hunks.iter().rev() {
        let previous: String = r.approved.blocks[h.old.clone()]
            .iter()
            .map(|b| b.html.as_str())
            .collect();
        let previous = (!previous.is_empty()).then_some(previous);
        if h.new.is_empty() {
            blocks.insert(
                h.new.start,
                Block {
                    id: format!("gone-{}", h.id),
                    html: String::new(),
                    change: Some("unreviewed"),
                    hunk: Some(h.id.clone()),
                    previous_html: previous,
                    changed_at: modified,
                },
            );
            continue;
        }
        for i in h.new.clone() {
            let b = &mut blocks[i];
            b.change = Some("unreviewed");
            b.hunk = Some(h.id.clone());
            b.changed_at = modified;
        }
        blocks[h.new.end - 1].previous_html = previous;
    }
}

fn get<'a>(snap: &'a Snapshot, id: &str) -> anyhow::Result<&'a Entry> {
    snap.docs
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("unknown document"))
}

pub fn mark_read(snap: &Snapshot, id: &str) -> anyhow::Result<()> {
    let e = get(snap, id)?;
    let sources: Vec<&str> = e.parsed.blocks.iter().map(|b| b.source.as_str()).collect();
    reads::mark(id, &sources)
}

fn tracked(e: &Entry) -> anyhow::Result<&review::Review> {
    e.review
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("this document is not tracked"))
}

pub fn accept(snap: &Snapshot, id: &str, hunk: &str) -> anyhow::Result<()> {
    let e = get(snap, id)?;
    review::accept(id, tracked(e)?, &e.text, &e.parsed, hunk)
}

pub fn revert(snap: &Snapshot, id: &str, hunk: &str) -> anyhow::Result<()> {
    let e = get(snap, id)?;
    review::revert(id, &e.path, tracked(e)?, &e.text, &e.parsed, hunk)
}

pub fn undo(snap: &Snapshot, id: &str) -> anyhow::Result<()> {
    let e = get(snap, id)?;
    tracked(e)?;
    review::undo(id, &e.path)
}

#[derive(serde::Serialize)]
pub struct Source {
    pub text: String,
    /// Version of the file on disk; a save must name it so it never overwrites newer edits.
    pub base: String,
}

fn version(text: &str) -> String {
    format!("{:x}", util::hash(text))
}

pub fn source(snap: &Snapshot, id: &str) -> anyhow::Result<Source> {
    let e = get(snap, id)?;
    Ok(Source {
        text: e.text.clone(),
        base: version(&e.text),
    })
}

/// Write the editor text. Saving also marks the document as read and, if tracked, approved.
pub fn save(snap: &Snapshot, id: &str, text: &str, base: &str) -> anyhow::Result<Source> {
    let e = get(snap, id)?;
    if version(&e.text) != base {
        anyhow::bail!("the file changed on disk while you were editing");
    }
    // The editor works with \n; keep the file's own line endings.
    let text = if e.text.contains("\r\n") {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_string()
    };
    std::fs::write(&e.path, &text)?;
    if e.review.is_some() {
        review::approve(id, &text)?;
    }
    let parsed = markdown::parse(&text);
    let sources: Vec<&str> = parsed.blocks.iter().map(|b| b.source.as_str()).collect();
    reads::mark(id, &sources)?;
    Ok(Source {
        base: version(&text),
        text,
    })
}

/// Rendered HTML for the editor preview, same renderer as the reader.
pub fn preview(text: &str) -> String {
    markdown::parse(text)
        .blocks
        .into_iter()
        .map(|b| b.html)
        .collect()
}
