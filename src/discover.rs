//! Finds agent Markdown in projects and in the agents' home folders.

use crate::model::DocKind;
use crate::util;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    "coverage",
    ".idea",
    ".vscode",
];
const MAX_DOCS: usize = 400;
const MAX_PLANS: usize = 30;

pub struct Found {
    pub path: PathBuf,
    pub kind: DocKind,
}

fn is_markdown(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("md" | "mdc" | "markdown")
    ) || p.file_name().is_some_and(|n| n == ".cursorrules")
}

/// Instruction files any supported agent reads.
pub fn is_instruction(rel: &Path) -> bool {
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let s = rel.to_string_lossy().replace('\\', "/");
    matches!(
        name,
        "CLAUDE.md"
            | "CLAUDE.local.md"
            | "AGENTS.md"
            | "AGENTS.override.md"
            | "GEMINI.md"
            | ".cursorrules"
            | "SKILL.md"
    ) || s.contains(".claude/rules/")
        || s.contains(".cursor/rules/")
        || s.ends_with(".github/copilot-instructions.md")
}

fn classify(rel: &Path) -> DocKind {
    if is_instruction(rel) {
        return DocKind::Instructions;
    }
    let s = rel.to_string_lossy().replace('\\', "/");
    if s.split('/')
        .any(|seg| matches!(seg, "plans" | "plan" | "specs" | "spec"))
    {
        DocKind::Plans
    } else {
        DocKind::Notes
    }
}

pub fn project_docs(root: &Path) -> Vec<Found> {
    let mut out = Vec::new();
    let walker = WalkDir::new(root)
        .max_depth(8)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && e.file_name()
                    .to_str()
                    .is_some_and(|n| SKIP_DIRS.contains(&n)))
        });
    for entry in walker.flatten() {
        if out.len() >= MAX_DOCS {
            break;
        }
        if !entry.file_type().is_file() || !is_markdown(entry.path()) {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        out.push(Found {
            path: entry.path().to_path_buf(),
            kind: classify(rel),
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Claude Code keeps per-project memory in ~/.claude/projects/<path with separators as dashes>/memory.
pub fn claude_memory_dir(root: &Path) -> Option<PathBuf> {
    let encoded: String = root
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let dir = util::home()?
        .join(".claude")
        .join("projects")
        .join(encoded)
        .join("memory");
    dir.is_dir().then_some(dir)
}

pub fn memory_docs(root: &Path) -> Vec<Found> {
    let Some(dir) = claude_memory_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<Found> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_markdown(p))
        .map(|path| Found {
            path,
            kind: DocKind::Notes,
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Files in the agents' home folders: global instructions, user rules, recent plans.
pub fn global_docs() -> Vec<Found> {
    let Some(home) = util::home() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for rel in [
        ".claude/CLAUDE.md",
        ".codex/AGENTS.md",
        ".codex/AGENTS.override.md",
        ".gemini/GEMINI.md",
    ] {
        let p = home.join(rel);
        if p.is_file() {
            out.push(Found {
                path: p,
                kind: DocKind::Instructions,
            });
        }
    }
    for entry in WalkDir::new(home.join(".claude/rules"))
        .max_depth(4)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() && is_markdown(entry.path()) {
            out.push(Found {
                path: entry.into_path(),
                kind: DocKind::Instructions,
            });
        }
    }
    let mut plans: Vec<(std::time::SystemTime, PathBuf)> =
        std::fs::read_dir(home.join(".claude/plans"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| is_markdown(&e.path()))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .collect();
    plans.sort_by_key(|a| std::cmp::Reverse(a.0));
    out.extend(plans.into_iter().take(MAX_PLANS).map(|(_, path)| Found {
        path,
        kind: DocKind::Plans,
    }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_name_and_folder() {
        assert_eq!(classify(Path::new("CLAUDE.md")), DocKind::Instructions);
        assert_eq!(classify(Path::new("src/AGENTS.md")), DocKind::Instructions);
        assert_eq!(
            classify(Path::new(".claude/rules/testing.md")),
            DocKind::Instructions
        );
        assert_eq!(classify(Path::new("docs/plans/v2.md")), DocKind::Plans);
        assert_eq!(classify(Path::new("README.md")), DocKind::Notes);
    }

    #[test]
    fn skips_build_folders() {
        let dir = std::env::temp_dir().join(format!("mieruka-discover-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("node_modules/x")).unwrap();
        std::fs::create_dir_all(dir.join("plans")).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "x").unwrap();
        std::fs::write(dir.join("node_modules/x/README.md"), "x").unwrap();
        std::fs::write(dir.join("plans/a.md"), "x").unwrap();
        let found = project_docs(&dir);
        let names: Vec<_> = found
            .iter()
            .map(|f| util::display(&f.path, Some(&dir)))
            .collect();
        assert_eq!(names, ["CLAUDE.md", "plans/a.md"]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
