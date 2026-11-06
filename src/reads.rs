//! "Changed since you read it": a snapshot of block hashes per document in ~/.mieruka/reads.

use crate::util;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    read_at: u64,
    blocks: Vec<String>,
}

fn dir() -> Option<PathBuf> {
    Some(util::home()?.join(".mieruka").join("reads"))
}

fn block_hash(source: &str) -> String {
    format!("{:x}", util::hash(source.trim()))
}

pub struct ReadState {
    /// Unix milliseconds of the last "Mark as read".
    pub read_at: Option<u64>,
    /// Indexes of blocks that are new or changed since then.
    pub changed: HashSet<usize>,
}

pub fn state(doc_id: &str, sources: &[&str]) -> ReadState {
    let snap = dir()
        .and_then(|d| util::read(&d.join(format!("{doc_id}.json"))))
        .and_then(|t| serde_json::from_str::<Snapshot>(&t).ok());
    let Some(snap) = snap else {
        return ReadState {
            read_at: None,
            changed: HashSet::new(),
        };
    };
    let known: HashSet<&str> = snap.blocks.iter().map(String::as_str).collect();
    let changed = sources
        .iter()
        .enumerate()
        .filter(|(_, s)| !known.contains(block_hash(s).as_str()))
        .map(|(i, _)| i)
        .collect();
    ReadState {
        read_at: Some(snap.read_at),
        changed,
    }
}

pub fn mark(doc_id: &str, sources: &[&str]) -> anyhow::Result<()> {
    let dir = dir().ok_or_else(|| anyhow::anyhow!("no home folder"))?;
    std::fs::create_dir_all(&dir)?;
    let read_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let snap = Snapshot {
        read_at,
        blocks: sources.iter().map(|s| block_hash(s)).collect(),
    };
    std::fs::write(
        dir.join(format!("{doc_id}.json")),
        serde_json::to_vec(&snap)?,
    )?;
    Ok(())
}
