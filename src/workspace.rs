//! Builds the whole workspace (projects, docs, contexts, issues) from disk.

use crate::checks::{self, CheckedDoc};
use crate::model::*;
use crate::{context, discover, markdown, reads, util};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Entry {
    pub meta: DocMeta,
    pub path: PathBuf,
    pub parsed: markdown::Parsed,
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

fn entry(path: PathBuf, kind: DocKind, project_id: &str, base: Option<&Path>) -> Option<Entry> {
    let text = util::read(&path)?;
    let parsed = markdown::parse(&text);
    let id = util::id_for(&path);
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
        read_at: state.read_at,
    };
    Some(Entry { meta, path, parsed })
}

pub fn build(roots: &[PathBuf]) -> Snapshot {
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

        let mut found = discover::project_docs(root);
        found.extend(discover::memory_docs(root));
        let mut ids = Vec::new();
        for f in found {
            let base = if f.path.starts_with(root) {
                Some(root.as_path())
            } else {
                None
            };
            if let Some(e) = entry(f.path, f.kind, &pid, base) {
                ids.push(e.meta.id.clone());
                order.push(e.meta.id.clone());
                docs.insert(e.meta.id.clone(), e);
            }
        }

        let doc_ids: HashMap<PathBuf, String> = ids
            .iter()
            .filter_map(|id| Some((docs[id].path.canonicalize().ok()?, id.clone())))
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
    let global = discover::global_docs();
    if !global.is_empty() {
        projects.push(Project {
            id: "home".into(),
            name: "Home".into(),
            root: "~".into(),
        });
        for f in global {
            if let Some(e) = entry(f.path, f.kind, "home", None) {
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
    let blocks = e
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
            }
        })
        .collect();
    Some(Doc {
        meta: e.meta.clone(),
        blocks,
    })
}

pub fn mark_read(snap: &Snapshot, id: &str) -> anyhow::Result<()> {
    let e = snap
        .docs
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("unknown document"))?;
    let sources: Vec<&str> = e.parsed.blocks.iter().map(|b| b.source.as_str()).collect();
    reads::mark(id, &sources)
}
