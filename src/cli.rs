//! Terminal commands: `check` for CI and `context` for a quick look.

use crate::model::{AgentContext, Issue, Severity};
use crate::workspace::{self, Snapshot};
use serde::Serialize;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum Format {
    Text,
    /// GitHub Actions annotations
    Github,
    Json,
}

struct Paint(bool);

impl Paint {
    /// NO_COLOR turns colors off, CLICOLOR_FORCE turns them on without a terminal (e.g. CI logs).
    fn new() -> Self {
        let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty() && v != "0");
        Paint(!set("NO_COLOR") && (set("CLICOLOR_FORCE") || std::io::stdout().is_terminal()))
    }
    fn wrap(&self, code: &str, s: &str) -> String {
        if self.0 {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
}

/// File path for output: relative to the current folder when possible, so CI annotations match.
fn shown_path(snap: &Snapshot, issue: &Issue) -> String {
    let Some(e) = snap.docs.get(&issue.doc_id) else {
        return String::new();
    };
    let cwd = std::env::current_dir().ok();
    let cwd = cwd.as_deref().and_then(|c| crate::util::canonical(c).ok());
    match cwd.as_deref().and_then(|c| e.path.strip_prefix(c).ok()) {
        Some(rel) => rel.to_string_lossy().replace('\\', "/"),
        None => crate::util::display(&e.path, None),
    }
}

fn severity(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "notice",
    }
}

/// GitHub workflow commands need %, CR and LF escaped (and , : in properties).
fn gh_escape(s: &str, property: bool) -> String {
    let s = s
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A");
    if property {
        s.replace(':', "%3A").replace(',', "%2C")
    } else {
        s
    }
}

#[derive(Serialize)]
struct JsonIssue<'a> {
    file: String,
    line: usize,
    severity: &'static str,
    rule: &'a str,
    title: &'a str,
    message: &'a str,
}

/// Returns the process exit code: 1 when any error or warning was found.
pub fn check(roots: &[PathBuf], format: Format) -> anyhow::Result<i32> {
    let snap = workspace::build(roots, false);
    let mut issues: Vec<(String, &Issue)> = snap
        .workspace
        .issues
        .iter()
        .map(|i| (shown_path(&snap, i), i))
        .collect();
    issues.sort_by(|a, b| (&a.0, a.1.line).cmp(&(&b.0, b.1.line)));
    let failing = issues
        .iter()
        .filter(|(_, i)| i.severity != Severity::Info)
        .count();

    match format {
        Format::Json => {
            let out: Vec<JsonIssue> = issues
                .iter()
                .map(|(file, i)| JsonIssue {
                    file: file.clone(),
                    line: i.line,
                    severity: severity(i.severity),
                    rule: i.rule,
                    title: i.title,
                    message: &i.message,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Github => {
            for (file, i) in &issues {
                println!(
                    "::{} file={},line={},title={}::{}",
                    severity(i.severity),
                    gh_escape(file, true),
                    i.line,
                    gh_escape(&format!("{} ({})", i.title, i.rule), true),
                    gh_escape(&i.message, false)
                );
            }
            summary(&snap, &Paint(false));
        }
        Format::Text => {
            let p = Paint::new();
            for (file, i) in &issues {
                let sev = match i.severity {
                    Severity::Error => p.wrap("31;1", "error  "),
                    Severity::Warning => p.wrap("33", "warning"),
                    Severity::Info => p.wrap("2", "info   "),
                };
                println!(
                    "{}  {}  {} {}",
                    p.wrap("1", &format!("{file}:{}", i.line)),
                    sev,
                    i.title,
                    p.wrap("2", &format!("[{}]", i.rule))
                );
                println!("    {}", i.message);
            }
            summary(&snap, &p);
        }
    }
    Ok(if failing > 0 { 1 } else { 0 })
}

fn summary(snap: &Snapshot, p: &Paint) {
    let files = snap.workspace.docs.len();
    let docs = format!("{files} agent {}", if files == 1 { "doc" } else { "docs" });
    let count = |sev: Severity| {
        snap.workspace
            .issues
            .iter()
            .filter(|i| i.severity == sev)
            .count()
    };
    let parts: Vec<String> = [
        (count(Severity::Error), "error", "errors"),
        (count(Severity::Warning), "warning", "warnings"),
        (count(Severity::Info), "note", "notes"),
    ]
    .into_iter()
    .filter(|(n, _, _)| *n > 0)
    .map(|(n, one, many)| plural(n, one, many))
    .collect();
    let line = if parts.is_empty() {
        p.wrap("32", &format!("No issues in {docs}."))
    } else {
        format!("{} in {docs}.", parts.join(", "))
    };
    // Keep stdout clean for annotations and pipes.
    eprintln!("{line}");
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn tokens(n: usize) -> String {
    if n < 1000 {
        n.to_string()
    } else {
        format!("{:.1}k", n as f64 / 1000.0)
    }
}

pub fn context(root: &Path, agent: Option<&str>, json: bool) -> anyhow::Result<()> {
    let snap = workspace::build(&[root.to_path_buf()], false);
    let contexts: Vec<&AgentContext> = snap
        .workspace
        .contexts
        .iter()
        .filter(|c| agent.is_none_or(|a| c.agent == a))
        .collect();
    if contexts.is_empty() {
        anyhow::bail!("unknown agent, use one of: claude, codex, cursor, copilot, antigravity");
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&contexts)?);
        return Ok(());
    }
    let p = Paint::new();
    for (n, c) in contexts.iter().enumerate() {
        if n > 0 {
            println!();
        }
        let start: usize = c
            .sources
            .iter()
            .filter(|s| !s.on_demand)
            .filter_map(|s| s.tokens)
            .sum();
        let approx = if c.approximate { "≈" } else { "" };
        println!(
            "{}  {}",
            p.wrap("1", &format!("{} · {}", c.label, c.dir)),
            p.wrap(
                "2",
                &if start == 0 {
                    "loads nothing at session start".to_string()
                } else {
                    format!("{approx}{} tokens at session start", tokens(start))
                }
            )
        );
        let width = c
            .sources
            .iter()
            .map(|s| s.path.chars().count())
            .max()
            .unwrap_or(0);
        let mut on_demand_header = false;
        for s in c
            .sources
            .iter()
            .filter(|s| s.tokens.is_some() && !s.on_demand)
            .chain(
                c.sources
                    .iter()
                    .filter(|s| s.tokens.is_some() && s.on_demand),
            )
            .chain(c.sources.iter().filter(|s| s.tokens.is_none()))
        {
            if s.on_demand && s.tokens.is_some() && !on_demand_header {
                println!("  {}", p.wrap("2", "loaded on demand:"));
                on_demand_header = true;
            }
            let t = match s.tokens {
                Some(t) => format!("{approx}{}", tokens(t)),
                None => "–".into(),
            };
            let line = format!("  {t:>7}  {:width$}  {}", s.path, s.how);
            println!(
                "{}",
                if s.tokens.is_none() {
                    p.wrap("2", &line)
                } else {
                    line
                }
            );
        }
    }
    if !json && contexts.iter().any(|c| c.agent == "claude") {
        eprintln!("Token counts are estimates: Anthropic doesn't publish Claude's tokenizer.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_github_commands() {
        assert_eq!(gh_escape("a,b:c%\n", true), "a%2Cb%3Ac%25%0A");
        assert_eq!(gh_escape("a,b: c", false), "a,b: c");
        assert_eq!(tokens(999), "999");
        assert_eq!(tokens(3700), "3.7k");
    }
}
