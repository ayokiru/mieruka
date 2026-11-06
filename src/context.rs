//! What each agent loads, following the vendors' documented rules
//! (code.claude.com/docs/en/memory, Codex AGENTS.md guide).

use crate::markdown;
use crate::model::ContextSource;
use crate::util;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const CLAUDE_IMPORT_HOPS: usize = 4;
const CLAUDE_FILES: [&str; 3] = ["CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md"];
const CLAUDE_MEMORY_LINES: usize = 200;
const CLAUDE_MEMORY_BYTES: usize = 25 * 1024;
const CODEX_DEFAULT_MAX_BYTES: usize = 32 * 1024;

pub struct ImportProblem {
    pub file: PathBuf,
    pub line: usize,
    pub target: String,
    pub too_deep: bool,
}

pub struct Built {
    pub sources: Vec<ContextSource>,
    /// Every file the agent reads up front, for duplicate detection.
    pub loaded: Vec<PathBuf>,
    pub import_problems: Vec<ImportProblem>,
}

struct Builder<'a> {
    base: &'a Path,
    doc_ids: &'a HashMap<PathBuf, String>,
    out: Built,
}

impl<'a> Builder<'a> {
    fn doc_id(&self, p: &Path) -> Option<String> {
        let c = p.canonicalize().ok()?;
        self.doc_ids.get(&c).cloned()
    }

    fn push(&mut self, path: &Path, how: String, tokens: Option<usize>, on_demand: bool) {
        let doc_id = self.doc_id(path);
        self.out.sources.push(ContextSource {
            path: util::display(path, Some(self.base)),
            how,
            tokens,
            on_demand,
            doc_id,
        });
        if tokens.is_some() && !on_demand {
            self.out
                .loaded
                .push(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
        }
    }

    fn skipped(&mut self, path: &Path, why: &str) {
        self.push(path, why.to_string(), None, false);
    }

    /// Adds a Claude memory file and expands its `@path` imports.
    fn claude_file(&mut self, path: &Path, how: String, on_demand: bool, hops: usize) {
        let Some(text) = util::read(path) else { return };
        let text = markdown::strip_html_comments(&text);
        self.push(path, how, Some(util::estimate_tokens(&text)), on_demand);
        let dir = path.parent().unwrap_or(Path::new("."));
        for (line, target) in markdown::parse(&text).imports {
            let resolved = resolve(dir, &target);
            if !resolved.is_file() {
                self.out.import_problems.push(ImportProblem {
                    file: path.to_path_buf(),
                    line,
                    target,
                    too_deep: false,
                });
                continue;
            }
            if hops >= CLAUDE_IMPORT_HOPS {
                self.out.import_problems.push(ImportProblem {
                    file: path.to_path_buf(),
                    line,
                    target: target.clone(),
                    too_deep: true,
                });
                self.skipped(&resolved, "not loaded: more than 4 import hops");
                continue;
            }
            let how = format!(
                "↳ imported by {}:{line}",
                util::display(path, Some(self.base))
            );
            self.claude_file(&resolved, how, on_demand, hops + 1);
        }
    }
}

pub fn resolve(dir: &Path, target: &str) -> PathBuf {
    if let Some(rest) = target.strip_prefix("~/")
        && let Some(h) = util::home()
    {
        return h.join(rest);
    }
    let p = Path::new(target);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        dir.join(p)
    }
}

/// Root first, `cwd` last.
fn ancestors(cwd: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = cwd.ancestors().map(Path::to_path_buf).collect();
    v.reverse();
    v
}

fn has_paths_frontmatter(text: &str) -> bool {
    text.starts_with("---")
        && text
            .lines()
            .skip(1)
            .take_while(|l| l.trim() != "---")
            .any(|l| l.trim_start().starts_with("paths:"))
}

fn rule_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.into_path())
        .collect();
    v.sort();
    v
}

fn managed_claude_md() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/ClaudeCode/CLAUDE.md")
    } else if cfg!(windows) {
        PathBuf::from(r"C:\Program Files\ClaudeCode\CLAUDE.md")
    } else {
        PathBuf::from("/etc/claude-code/CLAUDE.md")
    }
}

/// Skill names and descriptions from SKILL.md frontmatter; only these are loaded up front.
fn skill_summaries(dirs: &[PathBuf]) -> (usize, usize) {
    let mut count = 0;
    let mut tokens = 0;
    for dir in dirs {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let Some(text) = util::read(&entry.path().join("SKILL.md")) else {
                continue;
            };
            let front: String = text
                .lines()
                .skip(1)
                .take_while(|l| l.trim() != "---")
                .collect::<Vec<_>>()
                .join("\n");
            count += 1;
            tokens += util::estimate_tokens(&front);
        }
    }
    (count, tokens)
}

/// `instruction_files` are the project's instruction docs (absolute paths) found by discovery.
pub fn claude(
    root: &Path,
    instruction_files: &[PathBuf],
    doc_ids: &HashMap<PathBuf, String>,
) -> Built {
    let mut b = Builder {
        base: root,
        doc_ids,
        out: Built {
            sources: Vec::new(),
            loaded: Vec::new(),
            import_problems: Vec::new(),
        },
    };
    let home = util::home();

    let managed = managed_claude_md();
    if managed.is_file() {
        b.claude_file(&managed, "managed · always".into(), false, 0);
    }
    if let Some(h) = &home {
        b.claude_file(
            &h.join(".claude/CLAUDE.md"),
            "user · always".into(),
            false,
            0,
        );
        for rule in rule_files(&h.join(".claude/rules")) {
            let on_demand = util::read(&rule).is_some_and(|t| has_paths_frontmatter(&t));
            let how = if on_demand {
                "user rule · when matching files are read"
            } else {
                "user rule · always"
            };
            b.claude_file(&rule, how.into(), on_demand, 0);
        }
    }

    let chain = ancestors(root);
    let has_claude = chain
        .iter()
        .any(|d| CLAUDE_FILES.iter().any(|f| d.join(f).is_file()));
    for dir in &chain {
        let scope = if dir == root {
            "project"
        } else {
            "parent folder"
        };
        if has_claude {
            for f in CLAUDE_FILES {
                let p = dir.join(f);
                if p.is_file() {
                    b.claude_file(&p, format!("{scope} · always"), false, 0);
                }
            }
        } else {
            for f in ["AGENTS.md", ".claude/AGENTS.md"] {
                let p = dir.join(f);
                if p.is_file() {
                    b.claude_file(
                        &p,
                        format!("{scope} · AGENTS.md, no CLAUDE.md here"),
                        false,
                        0,
                    );
                }
            }
        }
    }

    for rule in rule_files(&root.join(".claude/rules")) {
        let on_demand = util::read(&rule).is_some_and(|t| has_paths_frontmatter(&t));
        let how = if on_demand {
            "rule · when matching files are read"
        } else {
            "rule · always"
        };
        b.claude_file(&rule, how.into(), on_demand, 0);
    }

    if let Some(mem) = crate::discover::claude_memory_dir(root).map(|d| d.join("MEMORY.md"))
        && let Some(text) = util::read(&mem)
    {
        let mut head: String = text
            .lines()
            .take(CLAUDE_MEMORY_LINES)
            .collect::<Vec<_>>()
            .join("\n");
        if head.len() > CLAUDE_MEMORY_BYTES {
            let mut cut = CLAUDE_MEMORY_BYTES;
            while !head.is_char_boundary(cut) {
                cut -= 1;
            }
            head.truncate(cut);
        }
        b.push(
            &mem,
            "auto memory · first 200 lines".into(),
            Some(util::estimate_tokens(&head)),
            false,
        );
    }

    let skill_dirs: Vec<PathBuf> = home
        .iter()
        .map(|h| h.join(".claude/skills"))
        .chain([root.join(".claude/skills")])
        .collect();
    let (count, tokens) = skill_summaries(&skill_dirs);
    if count > 0 {
        b.out.sources.push(ContextSource {
            path: format!("{count} skills"),
            how: "names and descriptions only".into(),
            tokens: Some(tokens),
            on_demand: false,
            doc_id: None,
        });
    }

    // Files below the working directory and files Claude Code never reads.
    for file in instruction_files {
        let Ok(rel) = file.strip_prefix(root) else {
            continue;
        };
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let dir = file.parent().unwrap_or(root);
        let below = rel.components().count() > 1
            && !rel.starts_with(".claude")
            && !rel.starts_with(".cursor")
            && !rel.starts_with(".github");
        let sub_has_claude = CLAUDE_FILES.iter().any(|f| dir.join(f).is_file());
        match name {
            "CLAUDE.md" | "CLAUDE.local.md" if below => {
                let folder = util::display(dir, Some(root));
                b.claude_file(
                    file,
                    format!("nested · when Claude works in {folder}/"),
                    true,
                    0,
                );
            }
            "AGENTS.md" if below && !has_claude && !sub_has_claude => {
                let folder = util::display(dir, Some(root));
                b.claude_file(
                    file,
                    format!("nested · when Claude works in {folder}/"),
                    true,
                    0,
                );
            }
            "AGENTS.md" if has_claude => {
                let imported = b
                    .out
                    .loaded
                    .iter()
                    .any(|p| file.canonicalize().is_ok_and(|c| &c == p));
                if !imported {
                    b.skipped(file, "not read: CLAUDE.md takes precedence");
                }
            }
            "AGENTS.override.md" | "GEMINI.md" | ".cursorrules" => {
                b.skipped(file, "not read by Claude Code")
            }
            _ if rel.starts_with(".cursor") || rel.ends_with(".github/copilot-instructions.md") => {
                b.skipped(file, "not read by Claude Code")
            }
            _ => {}
        }
    }
    b.out
}

fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| util::home().map(|h| h.join(".codex")))
}

fn codex_config(home: &Path) -> (Vec<String>, usize) {
    let Some(cfg) =
        util::read(&home.join("config.toml")).and_then(|t| t.parse::<toml::Table>().ok())
    else {
        return (Vec::new(), CODEX_DEFAULT_MAX_BYTES);
    };
    let fallbacks = cfg
        .get("project_doc_fallback_filenames")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let max = cfg
        .get("project_doc_max_bytes")
        .and_then(|v| v.as_integer())
        .map(|n| n as usize)
        .unwrap_or(CODEX_DEFAULT_MAX_BYTES);
    (fallbacks, max)
}

fn non_empty(p: &Path) -> Option<String> {
    util::read(p).filter(|t| !t.trim().is_empty())
}

pub fn codex(
    root: &Path,
    instruction_files: &[PathBuf],
    doc_ids: &HashMap<PathBuf, String>,
) -> Built {
    let mut b = Builder {
        base: root,
        doc_ids,
        out: Built {
            sources: Vec::new(),
            loaded: Vec::new(),
            import_problems: Vec::new(),
        },
    };
    let home = codex_home();
    let (fallbacks, max_bytes) = home
        .as_deref()
        .map(codex_config)
        .unwrap_or((Vec::new(), CODEX_DEFAULT_MAX_BYTES));

    if let Some(h) = &home {
        for name in ["AGENTS.override.md", "AGENTS.md"] {
            let p = h.join(name);
            if let Some(text) = non_empty(&p) {
                b.push(
                    &p,
                    "user · always".into(),
                    Some(util::estimate_tokens(&text)),
                    false,
                );
                break;
            }
        }
    }

    // The project root is the git root; without one Codex only checks the working directory.
    let git_root = root
        .ancestors()
        .find(|d| d.join(".git").exists())
        .map(Path::to_path_buf);
    let dirs: Vec<PathBuf> = match &git_root {
        Some(g) => ancestors(root)
            .into_iter()
            .filter(|d| d.starts_with(g))
            .collect(),
        None => vec![root.to_path_buf()],
    };
    let names: Vec<String> = ["AGENTS.override.md", "AGENTS.md"]
        .iter()
        .map(|s| s.to_string())
        .chain(fallbacks)
        .collect();
    let mut used = 0usize;
    for dir in &dirs {
        let Some((p, text)) = names
            .iter()
            .map(|n| dir.join(n))
            .find_map(|p| non_empty(&p).map(|t| (p, t)))
        else {
            continue;
        };
        if used >= max_bytes {
            b.skipped(&p, "not read: project_doc_max_bytes reached");
            continue;
        }
        let room = max_bytes - used;
        let take = text.len().min(room);
        used += take;
        let mut cut = take;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        let how = if take < text.len() {
            "project · cut at project_doc_max_bytes"
        } else {
            "project · always"
        };
        b.push(
            &p,
            how.into(),
            Some(util::estimate_tokens(&text[..cut])),
            false,
        );
    }

    for file in instruction_files {
        let Ok(rel) = file.strip_prefix(root) else {
            continue;
        };
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let below = rel.components().count() > 1;
        match name {
            "AGENTS.md" | "AGENTS.override.md" if below => {
                b.skipped(file, "not read: below the working directory")
            }
            "CLAUDE.md" | "CLAUDE.local.md" | "GEMINI.md" | ".cursorrules" => {
                b.skipped(file, "not read by Codex")
            }
            _ => {}
        }
    }
    b.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mieruka-ctx-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    fn project_files(root: &Path) -> Vec<PathBuf> {
        crate::discover::project_docs(root)
            .into_iter()
            .filter(|f| f.kind == crate::model::DocKind::Instructions)
            .map(|f| f.path)
            .collect()
    }

    #[test]
    fn claude_prefers_claude_md_and_follows_imports() {
        let root = temp("claude");
        std::fs::write(
            root.join("CLAUDE.md"),
            "# Rules\n\n@docs/style.md\n\n@missing.md\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/style.md"), "Use tabs.\n").unwrap();
        std::fs::write(root.join("AGENTS.md"), "Use pnpm.\n").unwrap();
        let built = claude(&root, &project_files(&root), &HashMap::new());
        let paths: Vec<_> = built
            .sources
            .iter()
            .map(|s| (s.path.as_str(), s.tokens.is_some()))
            .collect();
        assert!(paths.contains(&("CLAUDE.md", true)));
        assert!(paths.contains(&("docs/style.md", true)));
        assert!(paths.contains(&("AGENTS.md", false)));
        assert_eq!(built.import_problems.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn claude_reads_agents_md_without_claude_md() {
        let root = temp("agents");
        std::fs::write(root.join("AGENTS.md"), "Use pnpm.\n").unwrap();
        let built = claude(&root, &project_files(&root), &HashMap::new());
        let agents = built
            .sources
            .iter()
            .find(|s| s.path == "AGENTS.md")
            .unwrap();
        assert!(agents.tokens.is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn codex_walks_git_root_to_cwd_and_respects_limit() {
        let root = temp("codex");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("AGENTS.md"), "x".repeat(40 * 1024)).unwrap();
        std::fs::write(root.join("CLAUDE.md"), "claude only\n").unwrap();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/AGENTS.md"), "nested\n").unwrap();
        let built = codex(&root, &project_files(&root), &HashMap::new());
        let get = |p: &str| built.sources.iter().find(|s| s.path == p).unwrap();
        assert!(get("AGENTS.md").how.contains("cut"));
        assert!(get("CLAUDE.md").tokens.is_none());
        assert!(get("sub/AGENTS.md").tokens.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
