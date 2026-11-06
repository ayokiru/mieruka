//! JSON shapes shared with the UI (ui/src/data/types.ts).

use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    Instructions,
    Plans,
    Notes,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub rule: &'static str,
    pub title: &'static str,
    pub message: String,
    pub severity: Severity,
    pub doc_id: String,
    pub block_id: String,
    pub line: usize,
    /// Text inside the block to underline, if any.
    #[serde(skip)]
    pub needle: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub id: String,
    pub html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<&'static str>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DocMeta {
    pub id: String,
    pub project_id: String,
    pub path: String,
    pub kind: DocKind,
    pub lines: usize,
    pub title: String,
    pub changes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<u64>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Doc {
    #[serde(flatten)]
    pub meta: DocMeta,
    pub blocks: Vec<Block>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContextSource {
    pub path: String,
    pub how: String,
    /// None when the agent does not read the file.
    pub tokens: Option<usize>,
    pub on_demand: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentContext {
    pub agent: &'static str,
    pub label: &'static str,
    pub project_id: String,
    pub dir: String,
    pub approximate: bool,
    pub sources: Vec<ContextSource>,
}

#[derive(Serialize, Debug)]
pub struct Workspace {
    pub projects: Vec<Project>,
    pub docs: Vec<DocMeta>,
    pub issues: Vec<Issue>,
    pub contexts: Vec<AgentContext>,
}
