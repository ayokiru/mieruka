//! Cursor, GitHub Copilot and Antigravity, following their documentation:
//! cursor.com/docs/context/rules, docs.github.com (Copilot repository instructions),
//! antigravity.google/docs/rules.

use crate::context::{Builder, Built, ancestors};
use crate::model::Severity;
use crate::util;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const ANTIGRAVITY_FILE_BYTES: usize = 24_000;
const ANTIGRAVITY_BUDGET_TOKENS: usize = 20_000;

/// Simple YAML frontmatter: `key: value` lines, plus `- item` lists under an empty key.
pub fn frontmatter(text: &str) -> Option<HashMap<String, String>> {
    let rest = text.strip_prefix("---")?;
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))?;
    let end = rest.find("\n---").unwrap_or(rest.len());
    let mut out = HashMap::new();
    let mut last: Option<String> = None;
    for line in rest[..end].lines() {
        let t = line.trim();
        if let Some(item) = t.strip_prefix("- ")
            && let Some(k) = &last
        {
            let v: &mut String = out.entry(k.clone()).or_default();
            if !v.is_empty() {
                v.push_str(", ");
            }
            v.push_str(item.trim().trim_matches(['"', '\'']));
            continue;
        }
        if let Some((k, v)) = t.split_once(':') {
            let k = k.trim().to_string();
            out.insert(k.clone(), v.trim().trim_matches(['"', '\'']).to_string());
            last = Some(k);
        }
    }
    Some(out)
}

fn field<'a>(fm: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    fm.get(key)
        .map(|v| v.trim_matches(['[', ']']).trim())
        .filter(|v| !v.is_empty())
}

fn name_of(p: &Path) -> &str {
    p.file_name().and_then(|n| n.to_str()).unwrap_or_default()
}

fn folder(root: &Path, file: &Path) -> String {
    file.parent()
        .map(|d| util::display(d, Some(root)))
        .unwrap_or_default()
}

fn git_root(root: &Path) -> Option<PathBuf> {
    root.ancestors()
        .find(|d| d.join(".git").exists())
        .map(Path::to_path_buf)
}

fn tokens(text: &str) -> Option<usize> {
    Some(util::estimate_tokens(text))
}

pub fn cursor(root: &Path, files: &[PathBuf], doc_ids: &HashMap<PathBuf, String>) -> Built {
    let mut b = Builder::new(root, doc_ids);
    let rules = root.join(".cursor").join("rules");
    for f in files.iter().filter(|f| f.starts_with(&rules)) {
        let Some(text) = util::read(f) else { continue };
        if f.extension().and_then(|e| e.to_str()) != Some("mdc") {
            b.skipped(f, "not read: rules must be .mdc files");
            b.notice(
                f,
                "ignored-rule",
                "Ignored by Cursor",
                Severity::Warning,
                "Cursor reads only .mdc files in .cursor/rules. Rename it to .mdc and add frontmatter.",
            );
            continue;
        }
        let fm = frontmatter(&text).unwrap_or_default();
        if field(&fm, "alwaysApply") == Some("true") {
            b.push(f, "rule · always".into(), tokens(&text), false);
        } else if let Some(globs) = field(&fm, "globs") {
            let how = format!("rule · when files matching {globs} are in context");
            b.push(f, how, tokens(&text), true);
        } else if field(&fm, "description").is_some() {
            let how = "rule · when the agent finds it relevant".to_string();
            b.push(f, how, tokens(&text), true);
        } else {
            let how = "rule · manual, only when mentioned in chat".to_string();
            b.push(f, how, tokens(&text), true);
        }
    }
    for f in files {
        let Ok(rel) = f.strip_prefix(root) else {
            continue;
        };
        let nested = rel.components().count() > 1;
        match name_of(f) {
            "AGENTS.md" if !nested => {
                let text = util::read(f).unwrap_or_default();
                b.push(f, "project · always".into(), tokens(&text), false);
            }
            "AGENTS.md" if !f.starts_with(&rules) => {
                let text = util::read(f).unwrap_or_default();
                let how = format!("nested · when working in {}/", folder(root, f));
                b.push(f, how, tokens(&text), true);
            }
            ".cursorrules" if !nested => {
                let text = util::read(f).unwrap_or_default();
                let how = "legacy file, not in current Cursor docs".to_string();
                b.push(f, how, tokens(&text), false);
            }
            "CLAUDE.md" | "CLAUDE.local.md" => b.skipped(f, "not read by Cursor"),
            _ => {}
        }
    }
    b.out
}

pub fn copilot(root: &Path, files: &[PathBuf], doc_ids: &HashMap<PathBuf, String>) -> Built {
    let mut b = Builder::new(root, doc_ids);
    let repo = git_root(root).unwrap_or_else(|| root.to_path_buf());
    // Copilot works on the repository on GitHub, so git-ignored files never reach it.
    let ignored = crate::checks::gitignore(&repo);
    let local_only = |p: &Path| {
        p.starts_with(&repo) && ignored.matched_path_or_any_parents(p, false).is_ignore()
    };
    let main = repo.join(".github").join("copilot-instructions.md");
    if local_only(&main) {
        b.skipped(&main, "not read: git-ignored, so not on GitHub");
    } else if let Some(text) = util::read(&main) {
        let how = "repository · chat, cloud agent, code review".to_string();
        b.push(&main, how, tokens(&text), false);
    }
    let path_specific = repo.join(".github").join("instructions");
    for f in files.iter().filter(|f| f.starts_with(&path_specific)) {
        if !name_of(f).ends_with(".instructions.md") {
            continue;
        }
        if local_only(f) {
            b.skipped(f, "not read: git-ignored, so not on GitHub");
            continue;
        }
        let Some(text) = util::read(f) else { continue };
        let fm = frontmatter(&text).unwrap_or_default();
        let mut how = match field(&fm, "applyTo") {
            Some(globs) => format!("path-specific · when files match {globs}"),
            None => "path-specific · no applyTo pattern".to_string(),
        };
        if let Some(agent) = field(&fm, "excludeAgent") {
            how.push_str(&format!(" · not for {agent}"));
        }
        b.push(f, how, tokens(&text), true);
    }
    for f in files {
        let at_root = f.parent() == Some(repo.as_path());
        let relevant = matches!(name_of(f), "AGENTS.md" | "CLAUDE.md" | "GEMINI.md");
        if relevant && local_only(f) {
            b.skipped(f, "not read: git-ignored, so not on GitHub");
            continue;
        }
        match name_of(f) {
            "AGENTS.md" if at_root => {
                let text = util::read(f).unwrap_or_default();
                b.push(
                    f,
                    "agent instructions · cloud agent".into(),
                    tokens(&text),
                    false,
                );
            }
            "AGENTS.md" => {
                let text = util::read(f).unwrap_or_default();
                let how = format!(
                    "agent instructions · nearest one wins in {}/",
                    folder(root, f)
                );
                b.push(f, how, tokens(&text), true);
            }
            "CLAUDE.md" | "GEMINI.md" if at_root => {
                let text = util::read(f).unwrap_or_default();
                // The docs offer these as an alternative to AGENTS.md and don't say what happens with both.
                let how = if files.iter().any(|g| {
                    g.parent() == Some(repo.as_path())
                        && name_of(g) == "AGENTS.md"
                        && !local_only(g)
                }) {
                    "agent instructions · cloud agent; docs don't say if it loads next to AGENTS.md"
                } else {
                    "agent instructions · cloud agent, single-file option"
                }
                .to_string();
                b.push(f, how, tokens(&text), false);
            }
            "CLAUDE.md" | "GEMINI.md" => b.skipped(f, "not read: only the repository root counts"),
            "CLAUDE.local.md" => b.skipped(f, "not read by Copilot"),
            _ => {}
        }
    }
    b.out
}

/// Inline `@[label](path)` includes, then apply the per-file cut.
fn antigravity_text(path: &Path) -> Option<(String, bool)> {
    let raw = util::read(path)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut text = String::with_capacity(raw.len());
    let mut rest = raw.as_str();
    while let Some(at) = rest.find("@[") {
        text.push_str(&rest[..at]);
        let after = &rest[at..];
        let target = after
            .find("](")
            .and_then(|o| after[o + 2..].find(')').map(|c| &after[o + 2..o + 2 + c]));
        let end = after.find(')').map(|c| c + 1).unwrap_or(after.len());
        match target.and_then(|t| util::read(&crate::context::resolve(dir, t))) {
            Some(included) => text.push_str(&included),
            None => text.push_str(&after[..end]),
        }
        rest = &after[end..];
    }
    text.push_str(rest);
    if text.len() <= ANTIGRAVITY_FILE_BYTES {
        return Some((text, false));
    }
    let mut cut = ANTIGRAVITY_FILE_BYTES;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text.truncate(cut);
    Some((text, true))
}

fn antigravity_file(b: &mut Builder, path: &Path, scope: &str, always: &mut Vec<(usize, PathBuf)>) {
    let Some((text, cut)) = antigravity_text(path) else {
        return;
    };
    // An empty file adds nothing to the context.
    if text.trim().is_empty() {
        return;
    }
    let t = util::estimate_tokens(&text);
    let how = if cut {
        format!("{scope} · always · cut at 24,000 bytes")
    } else {
        format!("{scope} · always")
    };
    b.push(path, how, Some(t), false);
    always.push((t, path.to_path_buf()));
    if cut {
        b.notice(
            path,
            "antigravity-limit",
            "Cut by Antigravity",
            Severity::Warning,
            "Antigravity truncates rule files over 24,000 bytes, includes counted.",
        );
    }
}

fn antigravity_rule(b: &mut Builder, path: &Path, scope: &str, always: &mut Vec<(usize, PathBuf)>) {
    let Some((text, _)) = antigravity_text(path) else {
        return;
    };
    let fm = frontmatter(&text).unwrap_or_default();
    let t = Some(util::estimate_tokens(&text));
    match field(&fm, "trigger") {
        Some("always_on") => {
            antigravity_file(b, path, &format!("{scope} rule"), always);
        }
        Some("model_decision") => b.push(
            path,
            format!("{scope} rule · when relevant to the task"),
            t,
            true,
        ),
        Some("glob") => {
            let globs = field(&fm, "globs").or(field(&fm, "glob")).unwrap_or("?");
            b.push(
                path,
                format!("{scope} rule · when files match {globs}"),
                t,
                true,
            );
        }
        Some("manual") => b.push(
            path,
            format!("{scope} rule · manual, only when mentioned (not in the CLI)"),
            t,
            true,
        ),
        _ => {
            b.skipped(path, "not read: missing or unknown trigger");
            b.notice(
                path,
                "ignored-rule",
                "Discarded by Antigravity",
                Severity::Warning,
                "Rule files need YAML frontmatter with a trigger: always_on, model_decision, glob or manual. Without one, Antigravity silently drops the file.",
            );
        }
    }
}

fn md_children(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .collect();
    v.sort();
    v
}

pub fn antigravity(root: &Path, files: &[PathBuf], doc_ids: &HashMap<PathBuf, String>) -> Built {
    let mut b = Builder::new(root, doc_ids);
    let mut always: Vec<(usize, PathBuf)> = Vec::new();
    if let Some(home) = util::home() {
        let g = home.join(".gemini");
        for rel in [
            "AGENTS.md",
            "GEMINI.md",
            "config/AGENTS.md",
            "config/GEMINI.md",
        ] {
            let p = g.join(rel);
            if p.is_file() {
                antigravity_file(&mut b, &p, "user", &mut always);
            }
        }
        for p in md_children(&g.join("config").join("rules")) {
            antigravity_rule(&mut b, &p, "user", &mut always);
        }
    }

    // The workspace folder and its parents up to the repository root; nothing below it.
    let dirs: Vec<PathBuf> = match git_root(root) {
        Some(g) => ancestors(root)
            .into_iter()
            .filter(|d| d.starts_with(&g))
            .collect(),
        None => vec![root.to_path_buf()],
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    for dir in &dirs {
        let scope = if dir == root {
            "project"
        } else {
            "parent folder"
        };
        for rel in [
            "AGENTS.md",
            "GEMINI.md",
            ".agents/AGENTS.md",
            ".agents/GEMINI.md",
        ] {
            let p = dir.join(rel);
            if p.is_file() {
                antigravity_file(&mut b, &p, scope, &mut always);
                seen.push(p);
            }
        }
        for rules in [".agents/rules", ".agent/rules"] {
            for p in md_children(&dir.join(rules)) {
                antigravity_rule(&mut b, &p, scope, &mut always);
                seen.push(p);
            }
        }
    }
    for f in files {
        if seen.contains(f) {
            continue;
        }
        match name_of(f) {
            "AGENTS.md" | "GEMINI.md" => b.skipped(f, "not read: below the workspace folder"),
            "CLAUDE.md" | "CLAUDE.local.md" => b.skipped(f, "not read by Antigravity"),
            _ => {}
        }
    }

    let total: usize = always.iter().map(|(t, _)| t).sum();
    if total > ANTIGRAVITY_BUDGET_TOKENS
        && let Some((_, largest)) = always.iter().max_by_key(|(t, _)| *t)
    {
        b.notice(
            largest,
            "antigravity-budget",
            "Over Antigravity's budget",
            Severity::Info,
            format!(
                "Always-on rules add up to about {total} tokens. Above 20,000, Antigravity replaces the largest files with a one-line pointer."
            ),
        );
    }
    b.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mieruka-agents-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        util::canonical(&d).unwrap()
    }

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    fn how<'a>(b: &'a Built, path: &str) -> &'a str {
        &b.sources.iter().find(|s| s.path == path).unwrap().how
    }

    #[test]
    fn reads_frontmatter_lists() {
        let fm =
            frontmatter("---\nglobs:\n  - src/**\n  - \"*.ts\"\nalwaysApply: false\n---\nBody")
                .unwrap();
        assert_eq!(fm["globs"], "src/**, *.ts");
        assert_eq!(fm["alwaysApply"], "false");
        assert!(frontmatter("No frontmatter").is_none());
    }

    #[test]
    fn cursor_rule_types() {
        let root = temp("cursor");
        let files = vec![
            write(
                &root,
                ".cursor/rules/a.mdc",
                "---\nalwaysApply: true\n---\nA",
            ),
            write(
                &root,
                ".cursor/rules/b.mdc",
                "---\nglobs: src/**/*.ts\n---\nB",
            ),
            write(
                &root,
                ".cursor/rules/c.mdc",
                "---\ndescription: Testing\n---\nC",
            ),
            write(&root, ".cursor/rules/d.mdc", "D"),
            write(&root, ".cursor/rules/e.md", "E"),
            write(&root, "AGENTS.md", "root"),
            write(&root, "web/AGENTS.md", "nested"),
            write(&root, "CLAUDE.md", "claude"),
        ];
        let b = cursor(&root, &files, &HashMap::new());
        assert_eq!(how(&b, ".cursor/rules/a.mdc"), "rule · always");
        assert!(how(&b, ".cursor/rules/b.mdc").contains("src/**/*.ts"));
        assert!(how(&b, ".cursor/rules/c.mdc").contains("relevant"));
        assert!(how(&b, ".cursor/rules/d.mdc").contains("manual"));
        assert!(how(&b, ".cursor/rules/e.md").starts_with("not read"));
        assert_eq!(how(&b, "AGENTS.md"), "project · always");
        assert!(how(&b, "web/AGENTS.md").contains("web/"));
        assert_eq!(how(&b, "CLAUDE.md"), "not read by Cursor");
        assert_eq!(b.notices.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn copilot_files() {
        let root = temp("copilot");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let files = vec![
            write(&root, ".github/copilot-instructions.md", "Main"),
            write(
                &root,
                ".github/instructions/ts.instructions.md",
                "---\napplyTo: \"**/*.ts\"\n---\nTS",
            ),
            write(&root, "AGENTS.md", "root"),
            write(&root, "api/AGENTS.md", "nested"),
            write(&root, "api/CLAUDE.md", "nested claude"),
            write(&root, "CLAUDE.md", "ignored"),
        ];
        write(&root, ".gitignore", "CLAUDE.md\n");
        let b = copilot(&root, &files, &HashMap::new());
        assert!(how(&b, ".github/copilot-instructions.md").starts_with("repository"));
        assert!(how(&b, ".github/instructions/ts.instructions.md").contains("**/*.ts"));
        assert!(how(&b, "AGENTS.md").contains("cloud agent"));
        assert!(how(&b, "api/AGENTS.md").contains("nearest"));
        assert!(how(&b, "api/CLAUDE.md").starts_with("not read"));
        assert!(how(&b, "CLAUDE.md").contains("git-ignored"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn antigravity_rules_and_limits() {
        let root = temp("antigravity");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let files = vec![
            write(&root, "GEMINI.md", &"x".repeat(30_000)),
            write(
                &root,
                ".agents/rules/on.md",
                "---\ntrigger: always_on\n---\nOn",
            ),
            write(
                &root,
                ".agents/rules/ts.md",
                "---\ntrigger: glob\nglobs: \"*.ts\"\n---\nTS",
            ),
            write(&root, ".agents/rules/bad.md", "No frontmatter"),
            write(&root, "sub/AGENTS.md", "below"),
        ];
        let b = antigravity(&root, &files, &HashMap::new());
        assert!(how(&b, "GEMINI.md").contains("cut at 24,000 bytes"));
        assert_eq!(how(&b, ".agents/rules/on.md"), "project rule · always");
        assert!(how(&b, ".agents/rules/ts.md").contains("*.ts"));
        assert!(how(&b, ".agents/rules/bad.md").starts_with("not read"));
        assert!(how(&b, "sub/AGENTS.md").contains("below the workspace"));
        let rules: Vec<_> = b.notices.iter().map(|n| n.rule).collect();
        assert!(rules.contains(&"antigravity-limit") && rules.contains(&"ignored-rule"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
