//! Append-only JSONL spend ledger plus the cumulative credit cap.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{Result, WorldsError};

pub const DEFAULT_CAP: u64 = 4500;

pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    /// Ledger file inside `dir` (created on first append).
    pub fn in_dir(dir: &Path) -> Ledger {
        Ledger {
            path: dir.join("marble-ledger.jsonl"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// All readable entries, plus the number of unreadable lines.
    pub fn entries(&self) -> Result<(Vec<Value>, usize)> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
            Err(e) => return Err(e.into()),
        };
        let mut ok = Vec::new();
        let mut bad = 0;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<Value>(line) {
                Ok(v) => ok.push(v),
                Err(_) => bad += 1,
            }
        }
        Ok((ok, bad))
    }

    /// Credits committed by submitted generations.
    pub fn spent(&self) -> Result<u64> {
        let (entries, _) = self.entries()?;
        Ok(entries
            .iter()
            .filter(|e| e.get("event").and_then(Value::as_str) == Some("submit"))
            .filter_map(|e| e.get("est_credits").and_then(Value::as_u64))
            .sum())
    }

    pub fn append(&self, entry: &Value) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(entry)
            .map_err(|e| WorldsError::Io(format!("ledger encode: {e}")))?;
        writeln!(f, "{line}")?;
        f.flush()?;
        Ok(())
    }
}

/// Refuse when `spent + cost` would exceed `cap`.
pub fn check_cap(spent: u64, cost: u64, cap: u64) -> Result<()> {
    if spent.saturating_add(cost) > cap {
        return Err(WorldsError::CapExceeded(format!(
            "spend cap reached: {spent} of {cap} credits already committed, this world needs {cost}. \
             Raise SPLATCRAFT_MARBLE_CAP deliberately if you want to continue"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_ledger_has_zero_spend() {
        let d = tempfile::tempdir().unwrap();
        let l = Ledger::in_dir(d.path());
        assert_eq!(l.spent().unwrap(), 0);
        assert_eq!(l.entries().unwrap().0.len(), 0);
    }

    #[test]
    fn spend_sums_only_submits_and_tolerates_junk() {
        let d = tempfile::tempdir().unwrap();
        let l = Ledger::in_dir(d.path());
        l.append(&json!({"event": "submit", "est_credits": 1500}))
            .unwrap();
        l.append(&json!({"event": "note", "est_credits": 999}))
            .unwrap();
        l.append(&json!({"event": "submit", "est_credits": 1500}))
            .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(l.path())
            .unwrap()
            .write_all(b"not json\n")
            .unwrap();
        assert_eq!(l.spent().unwrap(), 3000);
        let (ok, bad) = l.entries().unwrap();
        assert_eq!((ok.len(), bad), (3, 1));
    }

    #[test]
    fn cap_logic() {
        assert!(check_cap(0, 1500, 4500).is_ok());
        assert!(check_cap(3000, 1500, 4500).is_ok());
        assert!(check_cap(3001, 1500, 4500).is_err());
        assert!(check_cap(u64::MAX, 1, 10).is_err());
    }
}
