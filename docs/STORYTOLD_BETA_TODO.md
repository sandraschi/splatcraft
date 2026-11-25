# Storytold Beta Readiness TODO - SplatCraft

**Target:** Submit to https://getartcraft.com/beta (Tier A Candidate #3 - 3D Gaussian Splatting toolkit)
**Audited:** 2026-11-21 (11.5k LOC, 128 tests, zero external sibling path deps)
**Reference:** `mcp-central-docs/integrations/storytold-artcraft/BETA_CANDIDATES.md`

---

## Already In Place
- [x] `LICENSE-MIT`, `LICENSE-APACHE`, `AGENTS.md`
- [x] Zero external `../sibling` path dependencies (fresh-clone safe)
- [x] 128 unit/integration tests + sample render (`plant-front.png`)

---

## P0 - Hard Submission Blockers (Must Complete Before Beta Form)

- [ ] **Legal & governance files**
  - [ ] Add `NOTICE` (explicit non-affiliation disclaimer).
  - [ ] Add `ATTRIBUTION.md` (all Rust crate and sample-splat/asset licenses).
  - [ ] Verify every dependency in `Cargo.lock` matches Storytold `standards/licences.md` SPDX allowlist.
- [ ] **Add `xtask` (`cargo xtask ci`)**: `fmt --check`, `clippy --all-targets -- -D warnings` (with `#![forbid(unsafe_code)]`, `unwrap_used`, `expect_used`, `panic`), `cargo test`, crate layering check, and license allowlist check.
- [ ] **Add GitHub Actions CI & Release workflows (`.github/workflows/ci.yml`, `release.yml`)**
  - Matrix build + test on `ubuntu-latest`, `macos-latest`, `windows-latest`.
  - Release workflow publishes cross-platform native binaries + `SHA256SUMS.txt`.
- [ ] **Add `packaging/env.sh`** (app ID, display name, binary names).

## P1 - Documentation, Naming & Feature Honesty

- [ ] **Add `docs/feature-names.md`** (RFC-0029 trademark hygiene audit across UI, menus, and README).
- [ ] **Add `docs/target-app-parity.md` and `docs/gaps.md`** (Implemented / Partial / Planned matrix for PLY/SPLAT/SPZ loading, cropping, color/opacity editing, camera paths, and export).
- [ ] **Sync `AGENTS.md`** with `storytold/craft-contrib/standards/app-agents.md` (RFC-0072).

## P2 - Polish, Accessibility & Evidence

- [ ] **Real desktop UI screenshots (dark + light modes)** in `docs/images/` linked from `README.md` (full app window showing viewport + scene controls, not only standalone render).
- [ ] **Accessibility check (RFC-0014)**: keyboard camera/editing shortcuts, numeric entry fields for transform/crop/exposure sliders, high-contrast light mode.
- [ ] **Clean fresh-clone build**: verify `cargo xtask ci` and `--smoke` on a fresh checkout with zero warnings.
- [ ] **Submit** 4-field form at https://getartcraft.com/beta (App name, Email, Repo URL, Description).
