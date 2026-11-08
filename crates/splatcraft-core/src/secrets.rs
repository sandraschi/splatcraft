//! Which secrets splatcraft knows and how they are stored.
//!
//! Deliberately NOT part of `dispatch` or the MCP tool list: an agent can never read, set or
//! delete a secret through the bus. Only the CLI (`splatcraft-cli secret ...`, typed by the
//! owner) writes them, and the worlds client reads them. Values are never returned.

use craft_secrets::{check_name, clean_value, resolve, OsStore, SecretError, SecretStore};
use serde_json::{json, Value};

use crate::error::{Error, Result};

/// One secret splatcraft can use.
pub struct Known {
    /// Short name used on the command line.
    pub name: &'static str,
    /// Environment variable that overrides the stored value.
    pub env: &'static str,
    /// Credential store service name.
    pub service: &'static str,
    /// Credential store entry name.
    pub entry: &'static str,
    /// What it is for, shown to the owner.
    pub purpose: &'static str,
}

pub const KNOWN: &[Known] = &[Known {
    name: "worlds",
    env: "WORLDLABS_API_KEY",
    service: "splatcraft",
    entry: "worldlabs-api-key",
    purpose: "World Labs Marble API key",
}];

impl From<SecretError> for Error {
    fn from(e: SecretError) -> Self {
        match e {
            SecretError::Invalid(m) => Error::Invalid(m),
            SecretError::Store(m) => Error::Io(format!("credential store: {m}")),
        }
    }
}

fn find(name: &str) -> Result<&'static Known> {
    KNOWN.iter().find(|k| k.name == name).ok_or_else(|| {
        let names: Vec<&str> = KNOWN.iter().map(|k| k.name).collect();
        Error::Invalid(format!(
            "unknown secret '{name}'; known: {}",
            names.join(", ")
        ))
    })
}

/// The World Labs key for the worlds client, from the environment or the credential store.
pub fn worlds_key() -> Result<String> {
    worlds_key_with(&OsStore)
}

pub fn worlds_key_with(store: &dyn SecretStore) -> Result<String> {
    let k = find("worlds")?;
    match resolve(store, k.env, k.service, k.entry)? {
        Some(s) => Ok(s.expose().to_string()),
        None => Err(Error::Auth(format!(
            "no World Labs API key: set {} or run `splatcraft-cli secret set worlds`",
            k.env
        ))),
    }
}

/// Which secrets are available and from where. Never includes a value.
pub fn status() -> Result<Value> {
    status_with(&OsStore)
}

pub fn status_with(store: &dyn SecretStore) -> Result<Value> {
    let mut rows = Vec::new();
    for k in KNOWN {
        let (set, source) = match resolve(store, k.env, k.service, k.entry) {
            Ok(Some(s)) => (true, json!(s.source.as_str())),
            Ok(None) => (false, Value::Null),
            Err(e) => return Err(e.into()),
        };
        rows.push(json!({
            "name": k.name, "purpose": k.purpose, "env": k.env,
            "set": set, "source": source,
        }));
    }
    Ok(json!({"success": true, "secrets": rows}))
}

/// Store a secret in the credential store. The caller supplies the value from a hidden prompt
/// or stdin; it is validated here and never echoed back.
pub fn set(name: &str, value: &str) -> Result<Value> {
    set_with(&OsStore, name, value)
}

pub fn set_with(store: &dyn SecretStore, name: &str, value: &str) -> Result<Value> {
    let k = find(name)?;
    check_name("service", k.service)?;
    let clean = clean_value(value)?;
    store.set(k.service, k.entry, &clean)?;
    Ok(json!({
        "success": true, "name": k.name, "stored": true, "length": clean.len(),
        "note": format!("stored in the OS credential store; {} still overrides it when set", k.env),
    }))
}

pub fn delete(name: &str) -> Result<Value> {
    delete_with(&OsStore, name)
}

pub fn delete_with(store: &dyn SecretStore, name: &str) -> Result<Value> {
    let k = find(name)?;
    let removed = store.delete(k.service, k.entry)?;
    Ok(json!({"success": true, "name": k.name, "removed": removed}))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use craft_secrets::MemStore;

    #[test]
    fn missing_key_names_both_ways_to_fix_it() {
        let _g = crate::ENV_LOCK.lock().unwrap();
        std::env::remove_var("WORLDLABS_API_KEY");
        let e = worlds_key_with(&MemStore::new()).unwrap_err();
        assert_eq!(e.error_type(), "auth");
        let m = e.to_string();
        assert!(m.contains("WORLDLABS_API_KEY") && m.contains("secret set worlds"));
    }

    #[test]
    fn stored_key_is_used_and_env_overrides_it() {
        let _g = crate::ENV_LOCK.lock().unwrap();
        std::env::remove_var("WORLDLABS_API_KEY");
        let s = MemStore::new();
        set_with(&s, "worlds", "  key-from-store \n").unwrap();
        assert_eq!(worlds_key_with(&s).unwrap(), "key-from-store");
        std::env::set_var("WORLDLABS_API_KEY", "key-from-env");
        assert_eq!(worlds_key_with(&s).unwrap(), "key-from-env");
        std::env::remove_var("WORLDLABS_API_KEY");
    }

    #[test]
    fn status_reports_source_and_never_the_value() {
        let _g = crate::ENV_LOCK.lock().unwrap();
        std::env::remove_var("WORLDLABS_API_KEY");
        let s = MemStore::new();
        let before = status_with(&s).unwrap();
        assert_eq!(before["secrets"][0]["set"], false);
        set_with(&s, "worlds", "super-secret-value").unwrap();
        let after = status_with(&s).unwrap();
        assert_eq!(after["secrets"][0]["set"], true);
        assert_eq!(after["secrets"][0]["source"], "credential_store");
        assert!(!after.to_string().contains("super-secret-value"));
    }

    #[test]
    fn set_validates_and_does_not_echo() {
        let s = MemStore::new();
        assert!(set_with(&s, "worlds", "   ").is_err());
        let e = set_with(&s, "worlds", "bad\nsecret-tail")
            .unwrap_err()
            .to_string();
        assert!(!e.contains("secret-tail"));
        let r = set_with(&s, "worlds", "abcdef").unwrap();
        assert_eq!(r["length"], 6);
        assert!(!r.to_string().contains("abcdef"));
        assert_eq!(
            set_with(&s, "nope", "x").unwrap_err().error_type(),
            "invalid"
        );
    }

    #[test]
    fn delete_reports_what_it_did() {
        let s = MemStore::new();
        assert_eq!(delete_with(&s, "worlds").unwrap()["removed"], false);
        set_with(&s, "worlds", "k").unwrap();
        assert_eq!(delete_with(&s, "worlds").unwrap()["removed"], true);
    }
}
