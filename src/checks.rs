//! Checks on instruction files. Conservative on purpose: a false alarm costs more trust than a miss.

use crate::context::Built;
use crate::markdown::Parsed;
use crate::model::{Issue, Severity};
use crate::util;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const CLAUDE_MD_LINES: usize = 200;
const PATH_EXTS: &[&str] = &[
    "md", "mdc", "sh", "ps1", "bat", "rs", "ts", "tsx", "js", "jsx", "mjs", "json", "toml", "yml",
    "yaml", "py", "txt", "lock", "cfg", "ini", "css", "html", "go", "java", "kt", "swift", "rb",
    "php", "cs",
];
const SKIP_FIRST: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".git",
    "vendor",
    ".venv",
];

pub struct CheckedDoc<'a> {
    pub id: &'a str,
    pub path: &'a Path,
    pub parsed: &'a Parsed,
    pub lines: usize,
    pub instruction: bool,
}

fn block_for(parsed: &Parsed, line: usize) -> String {
    let i = parsed
        .blocks
        .iter()
        .position(|b| b.start <= line && line <= b.end)
        .unwrap_or(0);
    format!("b{i}")
}

fn issue(
    doc: &CheckedDoc,
    rule: &'static str,
    title: &'static str,
    severity: Severity,
    line: usize,
    message: String,
    needle: Option<String>,
) -> Issue {
    let id = format!(
        "{:x}",
        util::hash(&format!(
            "{rule}|{}|{line}|{}",
            doc.id,
            needle.as_deref().unwrap_or("")
        ))
    );
    Issue {
        id,
        rule,
        title,
        message,
        severity,
        doc_id: doc.id.to_string(),
        block_id: block_for(doc.parsed, line),
        line,
        needle,
    }
}

/// A token from a code span or link that reads like a project path, normalized.
fn path_candidate(raw: &str) -> Option<String> {
    let t = raw
        .trim()
        .trim_end_matches(['.', ',', ':', ';'])
        .trim_start_matches("./");
    if t.is_empty()
        || t.contains(char::is_whitespace)
        || t.contains("://")
        || t.starts_with(['-', '@', '/', '#'])
        || t.contains([
            '*', '?', '<', '>', '{', '}', '$', '%', '|', '\\', '(', ')', '[', ']', '=',
        ])
        || t.contains(':')
        || !t.contains('/')
    {
        return None;
    }
    Some(t.to_string())
}

fn gitignore(root: &Path) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    let _ = b.add(root.join(".gitignore"));
    b.build().unwrap_or_else(|_| Gitignore::empty())
}

/// File names in the project, for "did you mean" hints.
fn file_index(root: &Path) -> HashMap<String, Vec<PathBuf>> {
    let mut idx: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let walker = walkdir::WalkDir::new(root)
        .max_depth(8)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && e.file_name()
                    .to_str()
                    .is_some_and(|n| SKIP_FIRST.contains(&n)))
        });
    for e in walker.flatten().take(20_000) {
        if e.file_type().is_file()
            && let Some(name) = e.file_name().to_str()
        {
            idx.entry(name.to_string()).or_default().push(e.into_path());
        }
    }
    idx
}

fn suggestion(root: &Path, missing: &str, idx: &HashMap<String, Vec<PathBuf>>) -> Option<String> {
    let p = Path::new(missing);
    let name = p.file_name()?.to_str()?;
    if let Some(found) = idx.get(name).and_then(|v| v.first()) {
        return Some(util::display(found, Some(root)));
    }
    // Same stem, other extension, same folder.
    let stem = p.file_stem()?.to_str()?;
    let dir = root.join(p.parent()?);
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|f| f.file_stem().and_then(|s| s.to_str()) == Some(stem))
        .map(|f| util::display(&f, Some(root)))
}

fn stale_paths(
    root: &Path,
    doc: &CheckedDoc,
    ignored: &Gitignore,
    idx: &HashMap<String, Vec<PathBuf>>,
    out: &mut Vec<Issue>,
) {
    let doc_dir = doc.path.parent().unwrap_or(root);
    let mut seen = HashSet::new();
    let candidates = doc
        .parsed
        .code_spans
        .iter()
        .map(|c| (c.line, c.text.clone(), true))
        .chain(
            doc.parsed
                .links
                .iter()
                .filter(|(_, u)| !u.contains(':') && !u.starts_with('#'))
                .map(|(l, u)| (*l, u.clone(), false)),
        );
    for (line, raw, is_code) in candidates {
        let Some(path) = path_candidate(&raw) else {
            continue;
        };
        let first = path.split('/').next().unwrap_or_default();
        let has_ext = Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| PATH_EXTS.contains(&e));
        if SKIP_FIRST.contains(&first) || !(root.join(first).exists() || has_ext) {
            continue;
        }
        let resolved = if let Some(rest) = path.strip_prefix("~/") {
            match util::home() {
                Some(h) => h.join(rest),
                None => continue,
            }
        } else {
            root.join(&path)
        };
        if resolved.exists() || doc_dir.join(&path).exists() {
            continue;
        }
        if resolved.starts_with(root)
            && ignored
                .matched_path_or_any_parents(&resolved, path.ends_with('/'))
                .is_ignore()
        {
            continue;
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        let hint = suggestion(root, &path, idx)
            .map(|s| format!(" Did you mean {s}?"))
            .unwrap_or_default();
        let needle = if is_code { Some(raw.clone()) } else { None };
        out.push(issue(
            doc,
            "stale-path",
            "Stale path",
            Severity::Warning,
            line,
            format!("{path} does not exist.{hint}"),
            needle,
        ));
    }
}

pub fn run(root: &Path, docs: &[CheckedDoc], claude: &Built, codex: &Built) -> Vec<Issue> {
    let mut out = Vec::new();
    let ignored = gitignore(root);
    let idx = file_index(root);
    let by_path: HashMap<PathBuf, &CheckedDoc> = docs
        .iter()
        .filter_map(|d| Some((d.path.canonicalize().ok()?, d)))
        .collect();
    let find = |p: &Path| p.canonicalize().ok().and_then(|c| by_path.get(&c).copied());

    for doc in docs.iter().filter(|d| d.instruction) {
        stale_paths(root, doc, &ignored, &idx, &mut out);
        let name = doc
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if matches!(name, "CLAUDE.md" | "CLAUDE.local.md") && doc.lines > CLAUDE_MD_LINES {
            out.push(issue(
                doc,
                "oversize",
                "Long file",
                Severity::Info,
                1,
                format!("{} lines. Anthropic suggests under {CLAUDE_MD_LINES}; move topic rules to .claude/rules/.", doc.lines),
                None,
            ));
        }
    }

    for p in &claude.import_problems {
        let Some(doc) = find(&p.file) else { continue };
        let (title, severity, message) = if p.too_deep {
            (
                "Import too deep",
                Severity::Warning,
                format!(
                    "@{} is more than 4 imports away, so Claude Code does not load it.",
                    p.target
                ),
            )
        } else {
            (
                "Broken import",
                Severity::Error,
                format!(
                    "@{} does not exist, so Claude Code loads nothing for it.",
                    p.target
                ),
            )
        };
        let rule = if p.too_deep {
            "import-depth"
        } else {
            "broken-import"
        };
        out.push(issue(
            doc,
            rule,
            title,
            severity,
            p.line,
            message,
            Some(format!("@{}", p.target)),
        ));
    }

    for s in &claude.sources {
        if s.how == "not read: CLAUDE.md takes precedence"
            && let Some(doc) = docs
                .iter()
                .find(|d| util::display(d.path, Some(root)) == s.path)
        {
            out.push(issue(
                    doc,
                    "shadowed",
                    "Not read by Claude Code",
                    Severity::Warning,
                    1,
                    "A CLAUDE.md in this folder or above takes precedence. Add @AGENTS.md to CLAUDE.md to load it.".into(),
                    None,
                ));
        }
    }
    for s in &codex.sources {
        if s.how.contains("cut at")
            && let Some(doc) = docs
                .iter()
                .find(|d| util::display(d.path, Some(root)) == s.path)
        {
            out.push(issue(
                doc,
                "codex-limit",
                "Cut by Codex",
                Severity::Warning,
                1,
                "Codex stops reading at project_doc_max_bytes (32 KiB by default).".into(),
                None,
            ));
        }
    }

    for (agent, built) in [("Claude Code", claude), ("Codex", codex)] {
        let mut seen = HashSet::new();
        for p in &built.loaded {
            if !seen.insert(p)
                && let Some(doc) = find(p)
            {
                out.push(issue(
                    doc,
                    "double-load",
                    "Loaded twice",
                    Severity::Warning,
                    1,
                    format!("{agent} reads this file twice (symlink or repeated import)."),
                    None,
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_candidates_skip_commands_and_globs() {
        assert_eq!(
            path_candidate("scripts/build.sh").as_deref(),
            Some("scripts/build.sh")
        );
        assert_eq!(
            path_candidate("./src/main.rs.").as_deref(),
            Some("src/main.rs")
        );
        for bad in [
            "cargo test",
            "src/**/*.rs",
            "https://x.y/z",
            "npm run lint:fix",
            "%APPDATA%\\x",
            "README.md",
            "-v/x",
        ] {
            assert!(path_candidate(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn flags_missing_paths_with_a_hint() {
        let root = std::env::temp_dir().join(format!("mieruka-checks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(root.join("scripts/build.sh"), "").unwrap();
        std::fs::write(root.join(".gitignore"), "out-of-tree/\n").unwrap();
        let text = "Run `scripts/build.ps1`, `scripts/build.sh` and `you/repo`.\n";
        std::fs::write(root.join("CLAUDE.md"), text).unwrap();
        let parsed = crate::markdown::parse(text);
        let path = root.join("CLAUDE.md");
        let doc = CheckedDoc {
            id: "d",
            path: &path,
            parsed: &parsed,
            lines: 1,
            instruction: true,
        };
        let empty = Built {
            sources: vec![],
            loaded: vec![],
            import_problems: vec![],
        };
        let empty2 = Built {
            sources: vec![],
            loaded: vec![],
            import_problems: vec![],
        };
        let issues = run(&root, &[doc], &empty, &empty2);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("Did you mean scripts/build.sh?"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
