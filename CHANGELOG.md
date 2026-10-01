# Changelog

## 1.2.8 — 2026-10-01

- Preserve the height of an already-open Herdr progress pane when native hooks
  reconnect to it. A user-adjusted pane no longer grows back to the saved size
  after each agent turn. `ap config set --pane-size` still resizes the exact
  owned pane immediately, and newly opened panes still use the saved size.


## 1.2.7 — 2026-10-01

- Default new progress windows to the measured Herdr minimum height of 10%.
  Existing explicitly saved pane sizes are preserved.
- Wait to open an automatic observer until the exact agent session has a goal
  or plan. Empty sessions keep the full source pane; manual `ap open` remains available.
- Detect a goal or plan added after Codex frontend startup, and open the optional
  tmux observer only when that session gains progress. Retain `AP_AUTO_OPEN=0`
  and `auto_open=false` opt-outs.


## 1.2.6 — 2026-09-30

- Add `ap setup` / `ap config setup` to choose a project-local progress-pane
  height on first use, including the installed Herdr minimum of 10%.
- Add `ap config set --pane-size PERCENT` (10–50) and YAML
  `pane_size_percent`; existing settings retain the 30% default.
- Apply size changes immediately to an exact owned Herdr or tmux observer
  when run from its source pane, preserving focus and unrelated layouts.

## 1.2.5 — 2026-09-30

- Split the Codex skill into `$ap-connect`, `$ap-theme`, and `$ap-recover`, while
  keeping `$ap` compatible with existing single-skill installations.
- Include all four skills in the macOS release archive and Homebrew package so
  they can be installed separately from the `ap` binary.

## 1.2.4 — 2026-09-30

- Wait briefly for concurrent project connection setup/removal locks before reporting
  them busy. File contents and receipts are still checked after the lock is acquired;
  user edits and existing backups remain protected.
- Address intermittent macOS lock contention observed during sequential connection
  setup/removal in release CI. A bounded fixture test covers Codex, Claude and OpenCode.

## 1.2.3 — 2026-09-30

- Prevent a manifest-free child project's native plan from being projected onto an
  ancestor product declaration. Existing product roots still own their declared plans.
- Persist a small passive-root marker when native input is received so historical
  sessions remain isolated after connector removal; managed connector receipts and
  nested Git roots also stop accidental ancestor selection.
- Concurrent first native hooks accept the same verified marker without losing an
  event; changed or unsafe marker bytes still fail closed.
- Preserve existing parent progress without automatic state rewrites. Exact nested
  three-agent fixtures cover start, follow and post-removal reads.

## 1.2.2 — 2026-09-30

- Allow passive Codex/Claude/OpenCode project hooks without creating ap.project.json;
  existing product declarations retain their mapping and Codex product MCP integration.
- Preserve an explicit AP_AUTO_OPEN=0 through all native launchers.
- Add read-only connection ownership state and safe next actions to connect preview,
  retaining prior JSON fields, user edits and protected apply/remove behavior.
- Align the ap skill with preview-based setup decisions; invalid, broken or foreign
  manifest links are not silently treated as absent declarations.
- Extend doctor with connection ownership, shell upgrade state, optional tmux availability
  and exact terminal-slot identity checks. Default reporting keeps exit 0; --strict returns
  nonzero when requested checks are unhealthy, without repairing user configuration.
- Shell status reports current/outdated/absent/partial/conflict and upgrade_required.
  Known legacy blocks can upgrade; edited blocks stay protected. Default bash selection
  handles prior owned login files as well as current login-file precedence.
- Retry short-lived shell file lock contention within a bounded timeout, preserving
  concurrent edits and exact backups.

## 1.2.1 — 2026-09-29

- bash/fish shell integration with exact backups, managed block protection and narrow removal.
  Default bash setup preserves login-file precedence and covers non-login rc files;
  legacy zsh blocks can be upgraded.
- Optional tmux automatic progress outside Herdr. Private launch identities, Codex frontend RPC
  selection and native hook ancestry prevent mixing concurrent sessions. Keep native focus,
  support initial above/below placement, and retain native exit status.
- Automatic local compatibility checks when agent versions change, including Codex RPC schemas.
  `ap compatibility --live` additionally tests authenticated plan response parsing; native task
  tools and complete pane lifecycle remain separate evidence. Failures never block native launch.
- Upgrade existing command setup with `ap shell install --shell zsh` (or the selected
  shell) after installing the new binary; the legacy block is backed up before replacement.

## 1.2.0 — 2026-09-29

- Opt-in zsh setup with `ap shell preview/install/status/remove` so ordinary interactive
  `codex` and `codex resume` commands use the existing client connection observer.
- Native executables, existing aliases/functions and unrelated rc edits are preserved.
  Setup backs up exact rc bytes, replaces atomically and refuses modified managed blocks.
- Outside Herdr and for non-interactive/management commands, invoke native Codex directly.
  Arguments are preserved; `command codex` bypasses the optional shell function.

## 1.1.1 — 2026-09-29

- Codex launcher observes its own frontend connection to the shared native server
  instead of forcing `--no-daemon`. Start/resume/fork replies identify the exact
  source process and session; raw transport bytes remain unchanged.
- Selection markers invalidate stale bindings when the same frontend changes threads.
  Unrelated thread reads and late responses cannot select a different source.
- Reuse managed observers across session changes, retaining focus, sizes and history.
- Include the concrete source-discovery failure in doctor output.
- Fresh native Codex/Claude Code/OpenCode plan and resume checks; see the verification
  report for exact versions and separate launcher coverage.

## 1.1.0 — 2026-09-28

- Signal, Forest, Ocean and Amber presets, custom preset inheritance and project-local
  YAML with anchors and merge keys.
- Background brightness from 0.25 to 2.0 and working, waiting, blocked, paused, error,
  done and empty palettes. Existing JSON colors remain compatible.
- Live palette updates retain the last valid settings after malformed edits.
- First GitHub source and macOS arm64 release; Developer signing and notarization
  remain unavailable.

## 1.0.2 — 2026-09-28

- Fix observer size restoration when exchanging a 90/10 split. Herdr caps each resize
  request at 0.5; apply multiple bounded requests and verify both original heights.

## 1.0.1 — 2026-09-28


- Project-local `#RRGGBB` controls for track, fill, accent, text, muted, metadata and warning
  colors. Running observers reload changes, preserve the last palette on malformed settings
  and support restoring Signal defaults.
- Connected native hooks automatically open the observer inside Herdr without requiring
  `AP_AUTO_OPEN=1` or the launcher opt-in. Proven source ownership and protected-layout
  guards still apply; `auto_open=false` and `AP_AUTO_OPEN=0` opt out.
- Above/below placement, CLI override and persisted preference. Owned vertical sibling
  observers can switch sides without recreating the terminal or changing focus.
- A viewer above the agent resolves its lower neighbor. Two possible source agents fail
  closed and require an explicit source pane.

## 1.0.0 — local acceptance candidate, 2026-09-28

Prepared from 0.5.0 after real three-agent integration, installation and recovery checks.
Native pane hooks prove process ownership, keep stdout as one valid hook JSON document,
and automatically open/reuse the observer when started with `ap launch`.
Human acceptance is recorded separately from packaging and automated verification.

## 0.5.0 — 2026-09-28 local candidate

- Claude Code native tasks/explicit progress reports and OpenCode native todos/assistant
  reports connect to the shared Plan/Task without copying raw conversations.
- Codex project hooks register exact transcript identity. Process ancestry proof rejects
  the wrong pane environment inherited by a shared daemon. `ap launch --agent codex` uses
  embedded Codex execution without changing user/global features settings.
- Explicit reconnect reuses an owned progress window across native session changes while
  retaining its receipt history. Other sources, changed terminals and busy windows are retained.
- Project-local hook/plugin setup supports preview, private backup and narrow removal.
  Installed adapters follow the public executable symlink across updates.
- Signal UI groups the goal and normal-size lime metric on a charcoal full-pane progress
  background. A paused goal keeps its planned current work visible. Plain text resume summary
  is available through `ap product summary`.
- Support evidence is for macOS 26.6.2 arm64, Herdr 0.9.0, Codex 0.157.1,
  Claude Code 2.1.274 and OpenCode 1.18.30. Developer signing/notarization and external
  publication are excluded. Final human acceptance is recorded separately.

## 0.4.0 — 2026-09-28 local candidate

Shared reporting/evidence policies, dependency/criteria/freshness tracking, explicit plan
changes, recoverable v2 storage, portable export/import, read-only MCP, diagnostics,
multi-product resume, fault/performance evidence and unsigned macOS installation package.

## 0.3.0 — 2026-09-25

Persistent product roadmap, Codex native goal/Plan projection, stable AP IDs and exact
session linkage. Child-plan completion does not accept whole product criteria.
