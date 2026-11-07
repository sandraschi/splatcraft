//! Find external tools without running them.
//!
//! Order: an explicit environment override, then PATH, then a tools
//! directory (`<tools>/<tool>/bin` and `<tools>/<tool>`, one level deep).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

fn exts() -> Vec<String> {
    if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(str::to_ascii_lowercase)
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![String::new()]
    }
}

fn is_file(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

/// Find `name` in the directories of `path_var`.
pub fn find_in(path_var: &OsStr, name: &str) -> Option<PathBuf> {
    let exts = exts();
    for dir in std::env::split_paths(path_var) {
        for ext in &exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if is_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Real executables only (no .bat/.cmd wrappers) in `dir`.
fn exe_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let p = dir.join(file);
    is_file(&p).then_some(p)
}

fn in_tools(tools: &Path, names: &[&str]) -> Option<PathBuf> {
    let mut subs: Vec<PathBuf> = std::fs::read_dir(tools)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subs.sort();
    for sub in subs {
        for dir in [sub.join("bin"), sub.clone()] {
            if let Some(p) = names.iter().find_map(|n| exe_in(&dir, n)) {
                return Some(p);
            }
        }
    }
    None
}

/// Locate a tool. A set-but-wrong override yields `None` on purpose: the
/// caller asked for a specific file, so falling back would hide the mistake.
pub fn locate(env_var: &str, names: &[&str], tools: Option<&Path>) -> Option<PathBuf> {
    if let Some(v) = std::env::var_os(env_var).filter(|v| !v.is_empty()) {
        let p = PathBuf::from(v);
        return is_file(&p).then_some(p);
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    if let Some(p) = names.iter().find_map(|n| find_in(&path_var, n)) {
        return Some(p);
    }
    tools.and_then(|t| in_tools(t, names))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn exe_name(n: &str) -> String {
        if cfg!(windows) {
            format!("{n}.exe")
        } else {
            n.to_string()
        }
    }

    #[test]
    fn empty_path_finds_nothing() {
        assert!(find_in(OsStr::new(""), "ffmpeg").is_none());
    }

    #[test]
    fn finds_a_file_in_a_dir() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(exe_name("faketool")), b"x").unwrap();
        let p = std::env::join_paths([d.path()]).unwrap();
        assert!(find_in(&p, "faketool").is_some());
        assert!(find_in(&p, "othertool").is_none());
    }

    #[test]
    fn tools_dir_prefers_bin_and_one_level() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("colmap-x").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(exe_name("zzcolmap")), b"x").unwrap();
        let found = in_tools(d.path(), &["zzcolmap"]).unwrap();
        assert!(found.starts_with(&bin));
        assert!(in_tools(d.path(), &["nothere"]).is_none());
        assert!(in_tools(&d.path().join("missing"), &["zzcolmap"]).is_none());
    }

    #[test]
    fn wrong_override_does_not_fall_back() {
        // PATH is not consulted when an override is set to a missing file.
        std::env::set_var("SPLATCRAFT_TEST_TOOL_X", "/definitely/not/here");
        assert!(locate("SPLATCRAFT_TEST_TOOL_X", &["cargo"], None).is_none());
        std::env::remove_var("SPLATCRAFT_TEST_TOOL_X");
    }
}
