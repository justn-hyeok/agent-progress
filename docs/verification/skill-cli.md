# Skill/CLI refinement — 2026-09-29

Base: bd5a875148eb746a42222821731ac597925fe9da; local worktree agent-progress-skill-cli.
This evidence accompanies the 1.2.2 candidate; publication and installation are tracked
separately in docs/current-plan.md.

User-requested Astra (gpt-6-astra, medium) reviewed the current contract, implemented
the bounded read-only status helper, and a separate Astra reviewed the integrated change.
Root reproduced failures, integrated code, authored regression tests and aligned skill/docs.

Reproduced with installed 1.2.1 in disposable fixtures:
- Codex/Claude/OpenCode connect preview failed without ap.project.json.
- All three launchers changed AP_AUTO_OPEN=0 to 1 in the native child's environment.

Verified with the local source build:
- Manifest-free three-agent preview/apply/native hook fixture/removal; no manifest,
  product goal or product state invented. Codex product MCP stays conditional on an
  existing declaration. Existing product mapping and protected removal regressions pass.
- Preview returns unmanaged/apply, managed/none or conflict/inspect while retaining
  its previous fields. Unrelated user additions are permitted; edited, duplicate,
  incomplete, unowned and symlink configurations are preserved and diagnosed.
  Static diagnostics do not disclose user setting values; previews do not write files.
- Native stubs verify explicit opt-out and default behavior for management/resume
  arguments, literal quoting and native exit status. The Herdr fixture verifies that
  an opted-out hook cannot create its observer, while the positive path still opens.
- Invalid, dangling or cross-root declaration links fail before derived state writes.
  The auto-discovery dangling-link failure was first reproduced, then fixed after
  independent Astra review. No global configuration, credentials or real sessions were read.

118 Rust tests, fmt/Clippy, release build, real tmux/PTY fixtures and source skill
validation passed. Commands: HERDR_ENV=0 cargo test --locked; cargo fmt --check;
cargo clippy --locked --all-targets -- -D warnings;
python3 scripts/pty_terminal.py target/release/ap;
skill-creator quick_validate.py skills/ap.

Doctor extension: connection ownership, optional valid/invalid product declaration and
cached-state identity, exact source/terminal-slot ownership, native version information,
tmux availability and selected shell setup. Its default exit remains report-only 0;
--strict fails only when healthy:false. Read-only fixture tests verify no latest source
selection, no global rc access, no user setting/request-arg disclosure and no AP file writes.
The shell check reports current/outdated/absent/partial/conflict and upgrade_required;
only the exact known legacy block upgrades. Edited or unreadable previously managed bash
files remain conflict and cannot be silently overwritten. A bounded lock wait handles
short-lived concurrent setup without weakening content rechecks or backups.

These native producers are fixtures; authenticated live producer and user acceptance
for the new refinement are not claimed. Release package/CI evidence is added after
the matching public source and assets have been verified.

1.2.2 local candidate: 129 Rust tests, fmt/Clippy, release build, release-binary
tmux/PTY and presentation scenarios, skill validation and exact archive installation
passed. Doctor checks are read-only: an absent declaration is optional, a declared
product without observations is not_observed, malformed or duplicate roadmap IDs and
cached objective mismatch are unhealthy. It diagnoses selected connection and shell
states, tmux availability and exact terminal-slot owner/session/cwd without reading a
focused/latest session. Normal report still exits 0; --strict fails for unhealthy
requested checks. Known legacy shell setup is upgradeable; edited or unreadable owned
blocks, including an inactive bash login file, are conflicts. Short-lived lock contention
is retried within two seconds, then content is checked again before replacement.
The exact installed archive fixture covered manifest-free connection, --strict,
all three shell statuses, tmux forwarding and narrow uninstall. Repacking was
byte-identical. Archive SHA-256:
a26b942618b62ea4ff3da97b64155d2a7649abdf50c58afc5a826b66505b1e43.
Authenticated native producer and human acceptance were not rerun for this batch.

Publication: v1.2.2 points to 0822dd5324e563f0c1f2fefba9e6c39bac175a7b.
The [source CI](https://github.com/justn-hyeok/agent-progress/actions/runs/36599817895)
and tag CI passed. The public archive download matched the SHA-256 above. Formula commit
691988cec8475c91377fbb729934387cbd6651e2 passed
[Homebrew install CI](https://github.com/justn-hyeok/agent-progress/actions/runs/36600288216).
Its [Rust CI](https://github.com/justn-hyeok/agent-progress/actions/runs/36600288191)
first hit a transient configuration-lock busy error (os error 35) in the existing
connection removal test, then passed the full rerun without changing source or tag.
Local brew upgrade/test/audit and the installed CLI's strict Codex/zsh doctor passed;
the installed binary hash matched the tested release binary. Global ap skill updates
were limited to baseline-matching files with private backups and passed validation.
