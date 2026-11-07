//! Path handling: input caps and optional root confinement.
//!
//! If `SPLATCRAFT_ROOT` is set, every path must lie inside it. The check is
//! lexical (`..` components are resolved without touching the disk), so it
//! also works for output files that do not exist yet.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::error::{Error, Result};

pub const MAX_ARG_CHARS: usize = 4096;

/// Fetch a required string argument, capped in length.
pub fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    let s = args
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Invalid(format!("missing string argument '{key}'")))?;
    if s.is_empty() {
        return Err(Error::Invalid(format!("'{key}' must not be empty")));
    }
    if s.chars().count() > MAX_ARG_CHARS {
        return Err(Error::Invalid(format!(
            "'{key}' exceeds {MAX_ARG_CHARS} characters"
        )));
    }
    if s.contains('\0') {
        return Err(Error::Invalid(format!("'{key}' contains a NUL byte")));
    }
    Ok(s)
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// True when `path` is inside `root` after lexical normalisation.
pub fn within(root: &Path, path: &Path) -> bool {
    normalize(path).starts_with(normalize(root))
}

/// Resolve a user path against the optional root.
pub fn resolve(raw: &str) -> Result<PathBuf> {
    let p = PathBuf::from(raw);
    if let Ok(root) = std::env::var("SPLATCRAFT_ROOT") {
        if !root.is_empty() {
            let root = PathBuf::from(root);
            let full = if p.is_absolute() { p } else { root.join(p) };
            if !within(&root, &full) {
                return Err(Error::Outside(format!(
                    "path '{raw}' is outside SPLATCRAFT_ROOT"
                )));
            }
            return Ok(normalize(&full));
        }
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn within_accepts_children_rejects_escape() {
        let root = Path::new("/data/splats");
        assert!(within(root, Path::new("/data/splats/a/b.ply")));
        assert!(!within(root, Path::new("/data/splats/../secret.ply")));
        assert!(!within(root, Path::new("/etc/passwd")));
    }

    #[test]
    fn str_arg_validates() {
        assert!(str_arg(&json!({}), "p").is_err());
        assert!(str_arg(&json!({"p": ""}), "p").is_err());
        assert!(str_arg(&json!({"p": "a\u{0}b"}), "p").is_err());
        let long = "x".repeat(MAX_ARG_CHARS + 1);
        assert!(str_arg(&json!({"p": long}), "p").is_err());
        assert_eq!(str_arg(&json!({"p": "ok"}), "p").unwrap(), "ok");
    }
}
