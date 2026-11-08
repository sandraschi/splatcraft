//! Secret lookup for craft apps.
//!
//! Resolution order: a non-blank environment variable wins, then the OS credential store
//! (Windows Credential Manager, macOS Keychain, Secret Service on Linux). Values are never
//! printed: `Secret` redacts itself in `Debug`, and nothing here returns a value except
//! `Secret::expose`, which callers use only to build a request.
//!
//! This crate has no app-specific code so other craft apps can depend on it unchanged.
//! See docs/SECRETS_STANDARD.md for the naming convention.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

/// Upper bound for a stored secret, in bytes.
pub const MAX_SECRET_LEN: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    /// The name or value is not acceptable.
    Invalid(String),
    /// The credential store failed or is unavailable.
    Store(String),
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecretError::Invalid(m) | SecretError::Store(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for SecretError {}

/// Where a resolved secret came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    Store,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Env => "env",
            Source::Store => "credential_store",
        }
    }
}

/// A secret value. `Debug` never shows it.
pub struct Secret {
    value: String,
    pub source: Source,
}

impl Secret {
    /// The raw value. Use only to build a request; never log or return it.
    pub fn expose(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret(<redacted>, source={})", self.source.as_str())
    }
}

/// A place secrets can live.
pub trait SecretStore {
    fn get(&self, service: &str, entry: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, service: &str, entry: &str, value: &str) -> Result<(), SecretError>;
    /// Returns whether something was removed.
    fn delete(&self, service: &str, entry: &str) -> Result<bool, SecretError>;
}

/// The OS credential store.
pub struct OsStore;

fn os_entry(service: &str, entry: &str) -> Result<keyring::Entry, SecretError> {
    check_name("service", service)?;
    check_name("entry", entry)?;
    keyring::Entry::new(service, entry).map_err(|e| SecretError::Store(e.to_string()))
}

impl SecretStore for OsStore {
    fn get(&self, service: &str, entry: &str) -> Result<Option<String>, SecretError> {
        match os_entry(service, entry)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError::Store(e.to_string())),
        }
    }

    fn set(&self, service: &str, entry: &str, value: &str) -> Result<(), SecretError> {
        os_entry(service, entry)?
            .set_password(value)
            .map_err(|e| SecretError::Store(e.to_string()))
    }

    fn delete(&self, service: &str, entry: &str) -> Result<bool, SecretError> {
        match os_entry(service, entry)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(SecretError::Store(e.to_string())),
        }
    }
}

type EntryMap = HashMap<(String, String), String>;

/// In-memory store for tests of this crate and of dependent apps.
#[derive(Default)]
pub struct MemStore {
    map: Mutex<EntryMap>,
}

impl MemStore {
    pub fn new() -> MemStore {
        MemStore::default()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, EntryMap>, SecretError> {
        self.map
            .lock()
            .map_err(|_| SecretError::Store("memory store lock poisoned".into()))
    }
}

impl SecretStore for MemStore {
    fn get(&self, service: &str, entry: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .lock()?
            .get(&(service.to_string(), entry.to_string()))
            .cloned())
    }

    fn set(&self, service: &str, entry: &str, value: &str) -> Result<(), SecretError> {
        self.lock()?
            .insert((service.to_string(), entry.to_string()), value.to_string());
        Ok(())
    }

    fn delete(&self, service: &str, entry: &str) -> Result<bool, SecretError> {
        Ok(self
            .lock()?
            .remove(&(service.to_string(), entry.to_string()))
            .is_some())
    }
}

/// Service and entry names: 1 to 64 chars of ASCII letters, digits, `-`, `_`, `.`.
pub fn check_name(what: &str, name: &str) -> Result<(), SecretError> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(SecretError::Invalid(format!(
            "{what} name must be 1 to 64 characters of letters, digits, '-', '_' or '.'"
        )))
    }
}

/// Trim and validate a secret before storing it. Never echoes the value in the error.
pub fn clean_value(value: &str) -> Result<String, SecretError> {
    let v = value.trim();
    if v.is_empty() {
        return Err(SecretError::Invalid("the secret is empty".into()));
    }
    if v.len() > MAX_SECRET_LEN {
        return Err(SecretError::Invalid(format!(
            "the secret is longer than {MAX_SECRET_LEN} bytes"
        )));
    }
    if v.chars().any(char::is_control) {
        return Err(SecretError::Invalid(
            "the secret contains control characters (a stray newline from pasting?)".into(),
        ));
    }
    Ok(v.to_string())
}

/// Environment variable first (blank counts as unset), then the store.
pub fn resolve(
    store: &dyn SecretStore,
    env_var: &str,
    service: &str,
    entry: &str,
) -> Result<Option<Secret>, SecretError> {
    if let Ok(v) = std::env::var(env_var) {
        let v = v.trim();
        if !v.is_empty() {
            return Ok(Some(Secret {
                value: v.to_string(),
                source: Source::Env,
            }));
        }
    }
    match store.get(service, entry)? {
        Some(v) if !v.trim().is_empty() => Ok(Some(Secret {
            value: v.trim().to_string(),
            source: Source::Store,
        })),
        _ => Ok(None),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn env_wins_over_store() {
        let s = MemStore::new();
        s.set("svc", "k", "from-store").unwrap();
        std::env::set_var("CRAFT_SECRETS_T1", "  from-env  ");
        let got = resolve(&s, "CRAFT_SECRETS_T1", "svc", "k")
            .unwrap()
            .unwrap();
        assert_eq!(got.expose(), "from-env");
        assert_eq!(got.source, Source::Env);
    }

    #[test]
    fn store_used_when_env_unset_or_blank() {
        let s = MemStore::new();
        s.set("svc", "k", "from-store").unwrap();
        let got = resolve(&s, "CRAFT_SECRETS_T2_UNSET", "svc", "k")
            .unwrap()
            .unwrap();
        assert_eq!((got.expose(), got.source), ("from-store", Source::Store));
        std::env::set_var("CRAFT_SECRETS_T3", "   ");
        let got = resolve(&s, "CRAFT_SECRETS_T3", "svc", "k")
            .unwrap()
            .unwrap();
        assert_eq!(got.source, Source::Store);
    }

    #[test]
    fn nothing_found_is_none() {
        let s = MemStore::new();
        assert!(resolve(&s, "CRAFT_SECRETS_T4_UNSET", "svc", "k")
            .unwrap()
            .is_none());
    }

    #[test]
    fn debug_never_shows_the_value() {
        let s = MemStore::new();
        s.set("svc", "k", "hunter2-super-secret").unwrap();
        let got = resolve(&s, "CRAFT_SECRETS_T5_UNSET", "svc", "k")
            .unwrap()
            .unwrap();
        let shown = format!("{got:?}");
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("redacted"));
    }

    #[test]
    fn values_are_validated_without_echoing_them() {
        assert!(clean_value("   ").is_err());
        assert!(clean_value("abc\ndef").is_err());
        assert!(clean_value(&"x".repeat(MAX_SECRET_LEN + 1)).is_err());
        assert_eq!(clean_value("  abc123  ").unwrap(), "abc123");
        let e = clean_value("bad\nvalue-secret").unwrap_err().to_string();
        assert!(!e.contains("value-secret"));
    }

    #[test]
    fn names_are_validated() {
        assert!(check_name("service", "splatcraft").is_ok());
        assert!(check_name("entry", "worldlabs-api-key").is_ok());
        assert!(check_name("entry", "").is_err());
        assert!(check_name("entry", "a b").is_err());
        assert!(check_name("entry", "../x").is_err());
    }

    #[test]
    fn memstore_delete_reports_whether_it_removed_something() {
        let s = MemStore::new();
        assert!(!s.delete("svc", "k").unwrap());
        s.set("svc", "k", "v").unwrap();
        assert!(s.delete("svc", "k").unwrap());
        assert_eq!(s.get("svc", "k").unwrap(), None);
    }

    /// Touches the real OS credential store. Run on purpose:
    /// `cargo test -p craft-secrets -- --ignored`.
    #[test]
    #[ignore]
    fn os_store_round_trip() {
        let store = OsStore;
        let (svc, entry) = ("craft-secrets-selftest", "roundtrip");
        store.delete(svc, entry).unwrap();
        assert_eq!(store.get(svc, entry).unwrap(), None);
        store.set(svc, entry, "selftest-value").unwrap();
        assert_eq!(
            store.get(svc, entry).unwrap().as_deref(),
            Some("selftest-value")
        );
        assert!(store.delete(svc, entry).unwrap());
        assert_eq!(store.get(svc, entry).unwrap(), None);
    }
}
