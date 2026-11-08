# Secrets standard for craft apps

Written for splatcraft, meant to be copied into every craft app (moneycraft and others) and
into the fleet standards. The reference implementation is the `craft-secrets` crate in this
repo; it has no splatcraft dependencies and can be lifted out as is.

## Rules

1. **Resolution order:** a non-blank environment variable wins, then the OS credential store
   (Windows Credential Manager, macOS Keychain, Secret Service on Linux). Nothing else: no
   dotfiles, no keys in committed config, no keys in MCP host config unless the owner chooses
   the env route knowingly.
2. **Naming:** credential store service = the app name (`splatcraft`); entry =
   `<provider>-api-key` (`worldlabs-api-key`); environment variable = the provider's own
   conventional name (`WORLDLABS_API_KEY`). One short CLI name per secret (`worlds`).
3. **The owner types it, the agent never touches it.** Setting and deleting are CLI-only:
   `<app>-cli secret set <name>` reads the value with hidden input (or one line of stdin for
   scripts). There is no MCP tool, no `dispatch` op and no app-bus message that reads, writes
   or lists values. An agent can run `secret status`, which reports only set/unset and the
   source (`env` or `credential_store`).
4. **Never echo.** Errors, logs, status output and `Debug` formatting never contain a value,
   not even a prefix or the length of a rejected one. Only the code that builds the request
   calls `Secret::expose`.
5. **Validate before storing:** trimmed, non-empty, at most 4096 bytes, no control characters
   (catches a pasted trailing newline).
6. **Missing secret is a typed `auth` error** that names both fixes: the env var and the
   `secret set` command.
7. **Keys that spend money** also get a dry-run default, an explicit confirm flag and a
   cumulative cap (see splatcraft `worlds`). A stored key is not a license to spend.
8. **Tests use `MemStore`.** One `#[ignore]` test touches the real store; run it on purpose
   with `cargo test -p craft-secrets -- --ignored`.

## Using the crate

```rust
use craft_secrets::{resolve, OsStore};

let key = resolve(&OsStore, "WORLDLABS_API_KEY", "splatcraft", "worldlabs-api-key")?
    .ok_or(/* auth error naming both fixes */)?;
client.header("WLT-Api-Key", key.expose());
```

Each app keeps a small `KNOWN` table (name, env var, service, entry, purpose) like
`crates/splatcraft-core/src/secrets.rs`, and a `secret` CLI subcommand with `status`, `set`
and `delete`.

## What this does not protect against

The credential store encrypts at rest, but any program running as the same OS user can read
it. That is acceptable for a revocable API key with a spend cap. It is not a reason to put
bank credentials, signing keys or anything irrevocable in an app like this; a finance craft
app needs its own threat review before storing such data. Rotate a key you suspect leaked.
On Linux the Secret Service needs a running keyring daemon.
