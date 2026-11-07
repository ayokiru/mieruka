use std::path::{Path, PathBuf};

/// FNV-1a: stable ids for files and blocks across runs.
pub fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

pub fn id_for(path: &Path) -> String {
    format!("{:012x}", hash(&path.to_string_lossy()) & 0xffff_ffff_ffff)
}

/// `canonicalize` without Windows' `\\?\` prefix, so paths look like the ones agents and people use.
pub fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    let c = path.canonicalize()?;
    #[cfg(windows)]
    {
        let s = c.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\")
            && rest.as_bytes().get(1) == Some(&b':')
        {
            return Ok(PathBuf::from(rest));
        }
    }
    Ok(c)
}

pub fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

/// Rough token count. Claude has no public tokenizer; ~3.6 characters per token fits English and code.
pub fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() as f64 / 3.6).ceil() as usize
}

/// Path for display: relative to `base` when inside it, `~/...` inside home, absolute otherwise.
pub fn display(path: &Path, base: Option<&Path>) -> String {
    if let Some(rel) = base.and_then(|b| path.strip_prefix(b).ok()) {
        return rel.to_string_lossy().replace('\\', "/");
    }
    if let Some(rel) = home().and_then(|h| path.strip_prefix(&h).ok().map(Path::to_path_buf)) {
        return format!("~/{}", rel.to_string_lossy().replace('\\', "/"));
    }
    path.to_string_lossy().into_owned()
}

pub fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}
