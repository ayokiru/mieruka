//! Unreviewed changes: an approved copy of each tracked file lives in ~/.mieruka/approved.
//! Edits made outside mieruka show up as hunks that can be accepted or reverted.

use crate::markdown::{self, Parsed, ParsedBlock};
use crate::util;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub struct Hunk {
    /// Hash of both sides: stays the same while neither text changes, so stale clicks fail.
    pub id: String,
    /// Block indexes in the approved text.
    pub old: Range<usize>,
    /// Block indexes in the current text.
    pub new: Range<usize>,
}

pub struct Review {
    pub approved_text: String,
    pub approved: Parsed,
    pub hunks: Vec<Hunk>,
}

fn store(kind: &str, doc_id: &str) -> Option<PathBuf> {
    Some(
        util::home()?
            .join(".mieruka")
            .join(kind)
            .join(format!("{doc_id}.md")),
    )
}

fn write(path: &Path, text: &str) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)?;
    Ok(())
}

/// The first time a file is seen, its current text becomes the approved version.
pub fn load(doc_id: &str, text: &str, parsed: &Parsed) -> Option<Review> {
    let path = store("approved", doc_id)?;
    let approved_text = match util::read(&path) {
        Some(t) => t,
        None => {
            write(&path, text).ok()?;
            text.to_string()
        }
    };
    let approved = markdown::parse(&approved_text);
    let hunks = hunks(&approved.blocks, &parsed.blocks);
    Some(Review {
        approved_text,
        approved,
        hunks,
    })
}

/// Edits saved in mieruka are reviewed by definition: the whole saved text becomes approved.
pub fn approve(doc_id: &str, text: &str) -> anyhow::Result<()> {
    let path = store("approved", doc_id).ok_or_else(|| anyhow::anyhow!("no home folder"))?;
    write(&path, text)
}

fn same(a: &ParsedBlock, b: &ParsedBlock) -> bool {
    a.source.trim() == b.source.trim()
}

/// Block-level diff (longest common subsequence). Documents are small, so O(n·m) is fine.
pub fn hunks(old: &[ParsedBlock], new: &[ParsedBlock]) -> Vec<Hunk> {
    let (n, m) = (old.len(), new.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if same(&old[i], &new[j]) {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j, mut si, mut sj) = (0, 0, 0, 0);
    let mut push = |o: Range<usize>, n: Range<usize>| {
        if !o.is_empty() || !n.is_empty() {
            let mut key = String::new();
            for b in &old[o.clone()] {
                key.push_str(&b.source);
                key.push('\0');
            }
            key.push('\x01');
            for b in &new[n.clone()] {
                key.push_str(&b.source);
                key.push('\0');
            }
            out.push(Hunk {
                id: format!("{:x}", util::hash(&key)),
                old: o,
                new: n,
            });
        }
    };
    while i < n || j < m {
        if i < n && j < m && same(&old[i], &new[j]) {
            push(si..i, sj..j);
            i += 1;
            j += 1;
            si = i;
            sj = j;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            j += 1;
        } else {
            i += 1;
        }
    }
    push(si..n, sj..m);
    out
}

/// Raw source of a run of blocks, including the blank lines between them.
fn slice(text: &str, blocks: &[ParsedBlock], range: Range<usize>) -> String {
    if range.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let s = blocks[range.start].start - 1;
    let e = blocks[range.end - 1].end.min(lines.len());
    lines[s..e].join("\n")
}

/// Replace a run of blocks in `text` with `insert` (empty run: insert before it; empty insert: delete).
fn splice(text: &str, blocks: &[ParsedBlock], range: Range<usize>, insert: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    let mut chunk: Vec<&str> = insert.lines().collect();
    if !range.is_empty() {
        let mut s = blocks[range.start].start - 1;
        let mut e = blocks[range.end - 1].end.min(lines.len());
        if chunk.is_empty() && (s == 0 || lines[s - 1].trim().is_empty()) {
            // Also drop the blank lines that separated the removed blocks from their neighbours.
            while e < lines.len() && lines[e].trim().is_empty() {
                e += 1;
            }
            if e == lines.len() {
                while s > 0 && lines[s - 1].trim().is_empty() {
                    s -= 1;
                }
            }
        }
        lines.splice(s..e, chunk);
    } else if !chunk.is_empty() {
        let at = blocks
            .get(range.start)
            .map(|b| b.start - 1)
            .unwrap_or(lines.len());
        if at < lines.len() {
            chunk.push("");
        } else if at > 0 && !lines[at - 1].trim().is_empty() {
            chunk.insert(0, "");
        }
        lines.splice(at..at, chunk);
    }
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = lines.join(eol);
    if text.ends_with('\n') || text.is_empty() {
        out.push_str(eol);
    }
    out
}

fn find<'a>(review: &'a Review, hunk_id: &str) -> anyhow::Result<&'a Hunk> {
    review
        .hunks
        .iter()
        .find(|h| h.id == hunk_id)
        .ok_or_else(|| anyhow::anyhow!("the file changed again, reload to see the latest version"))
}

/// Make the current text of one hunk part of the approved version.
pub fn accept(
    doc_id: &str,
    review: &Review,
    text: &str,
    parsed: &Parsed,
    hunk_id: &str,
) -> anyhow::Result<()> {
    let h = find(review, hunk_id)?;
    let next = splice(
        &review.approved_text,
        &review.approved.blocks,
        h.old.clone(),
        &slice(text, &parsed.blocks, h.new.clone()),
    );
    let path = store("approved", doc_id).ok_or_else(|| anyhow::anyhow!("no home folder"))?;
    write(&path, &next)
}

/// Put the approved text of one hunk back into the file. The previous text is kept for one undo.
pub fn revert(
    doc_id: &str,
    file: &Path,
    review: &Review,
    text: &str,
    parsed: &Parsed,
    hunk_id: &str,
) -> anyhow::Result<()> {
    let h = find(review, hunk_id)?;
    let next = splice(
        text,
        &parsed.blocks,
        h.new.clone(),
        &slice(
            &review.approved_text,
            &review.approved.blocks,
            h.old.clone(),
        ),
    );
    let backup = store("undo", doc_id).ok_or_else(|| anyhow::anyhow!("no home folder"))?;
    write(&backup, text)?;
    std::fs::write(file, next)?;
    Ok(())
}

/// Undo the last revert of this document.
pub fn undo(doc_id: &str, file: &Path) -> anyhow::Result<()> {
    let backup = store("undo", doc_id).ok_or_else(|| anyhow::anyhow!("no home folder"))?;
    let text = util::read(&backup).ok_or_else(|| anyhow::anyhow!("nothing to undo"))?;
    std::fs::write(file, text)?;
    std::fs::remove_file(backup)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_all(old: &str, new: &str, accept_side: bool) -> String {
        let (po, pn) = (markdown::parse(old), markdown::parse(new));
        let hs = hunks(&po.blocks, &pn.blocks);
        // Apply from the end so earlier block indexes stay valid.
        let mut out = if accept_side {
            old.to_string()
        } else {
            new.to_string()
        };
        for h in hs.iter().rev() {
            out = if accept_side {
                splice(
                    &out,
                    &markdown::parse(&out).blocks,
                    h.old.clone(),
                    &slice(new, &pn.blocks, h.new.clone()),
                )
            } else {
                splice(
                    &out,
                    &markdown::parse(&out).blocks,
                    h.new.clone(),
                    &slice(old, &po.blocks, h.old.clone()),
                )
            };
        }
        out
    }

    const OLD: &str = "# Title\n\nKeep this.\n\n- a\n- b\n\nRemoved later.\n\nEnd.\n";
    const NEW: &str =
        "# Title\n\nKeep this.\n\n- a\n- b\n- c\n\nAdded paragraph.\n\nEnd.\n\nTail.\n";

    #[test]
    fn finds_hunks() {
        let (po, pn) = (markdown::parse(OLD), markdown::parse(NEW));
        let hs = hunks(&po.blocks, &pn.blocks);
        assert_eq!(hs.len(), 2);
        assert_eq!((hs[0].old.clone(), hs[0].new.clone()), (2..4, 2..4));
        assert_eq!((hs[1].old.clone(), hs[1].new.clone()), (5..5, 5..6));
        assert!(hunks(&po.blocks, &po.blocks).is_empty());
    }

    #[test]
    fn accept_and_revert_round_trip() {
        assert_eq!(apply_all(OLD, NEW, true), NEW);
        assert_eq!(apply_all(OLD, NEW, false), OLD);
    }

    #[test]
    fn deletes_and_inserts_cleanly() {
        let old = "One.\n\nTwo.\n\nThree.\n";
        let new = "One.\n\nThree.\n";
        assert_eq!(apply_all(old, new, true), new);
        assert_eq!(apply_all(old, new, false), old);
        assert_eq!(apply_all("A.\n", "New.\n\nA.\n", true), "New.\n\nA.\n");
        assert_eq!(apply_all("A.\r\n\r\nB.\r\n", "A.\r\n", true), "A.\r\n");
    }
}
