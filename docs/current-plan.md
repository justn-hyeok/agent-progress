# Current release plan

## Skill bundle release — 2026-09-30

- [x] [AP-19/skill-package] Include `$ap` and the three focused skills in the 1.2.5 archive and Homebrew formula.
- [x] [AP-09/skill-release-check] Verify the source, package checksum, exact skill bytes, local update/rollback and terminal fixtures.
- [x] [AP-19/skill-release] Publish the matching 1.2.5 tag, assets and Homebrew formula, then verify the public install.

Candidate archive: `agent-progress-1.2.5-macos-arm64.tar.gz`, SHA-256
`01af4d0d6327aeda74f41aed53e0713c27d80fd98d05a3451abcd8ac0cc2ed68`.
The reproducible repack was byte-identical. Rust tests, fmt, Clippy, skill
validation, release-binary PTY presentation and terminal fixtures passed with
the same `HERDR_ENV=0` isolation used by CI. The first local test attempt
inherited the active Herdr environment and its bridge fixtures failed; the
documented CI environment passed. The exact archive contained all four skills
and passed member checksums. An isolated install upgraded 1.2.4 to 1.2.5,
rolled back to 1.2.4, and uninstalled cleanly. Full package smoke reached
real Codex and Claude native producers, then OpenCode's first
`opencode-go/glm-5.3-flash` model request timed out at 120 seconds. A later
bounded OpenCode retest is recorded below.
The released binary logic is unchanged from 1.2.4.
Published release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.5.
The annotated tag resolves to source commit b9e8064d845164662698012464d923cbecfb6415;
its Rust CI passed (36612926812), as did the source push CI (36612630502).
The public archive and checksum asset matched the local candidate SHA-256.
Formula commit a303efa675b4ec1a1879aac68a843c80c57c7516 passed Homebrew
install CI (36613053687) and Rust CI (36613051967). This Mac upgraded via
Homebrew to 1.2.5, and the
installed binary and four `share/agent-progress/skills` folders matched the
verified source bytes. `brew test`, `brew audit --strict`, installed doctor
`--shell zsh --strict` and local Codex skill validation passed. Local Codex
skills were backed up and updated to the matching 1.2.5 source. Homebrew's
automatic cleanup removed the older local Cellar copies; project data and
running panes were not changed.

Follow-up OpenCode 1.18.30 live check used the installed ap 1.2.5 and
`opencode-go/gpt-6-luna` in a private disposable project. Native `todowrite`
was observed at 0/2 → 1/2 → 2/2, with stable task IDs, the same exact session
resumed in a new process, one bridge stream, and a native test-file edit.
`ap compatibility --agent opencode` also passed its CLI interface check.
The model skipped `todowrite` on the first progress prompt (state stayed 0/2);
one explicit corrective prompt in that same session produced 1/2, and the
next process completed 2/2. This proves the native connection and state path,
not reliable tool selection for every prompt or the timed-out flash model.
Raw session and provider records remain private and outside Git.

## Focused ap skills — 2026-09-30

- [x] [AP-06/skill-split] Split connection, theme and recovery into separately invokable skills while retaining the existing `$ap` entrypoint.
- [x] [AP-09/skill-split] Validate source and local installed copies against ap 1.2.4 help and the skill validator.
- [x] [AP-19/skill-split] Update README installation and invocation guidance; record local installation state.

`skills/ap-connect`, `skills/ap-theme` and `skills/ap-recover` are standalone
Codex skills. The existing `skills/ap` keeps its detailed references so earlier
single-skill installations continue to work. The four source folders passed
skill-creator validation; the locally installed copies matched the source
byte-for-byte and passed the same validation. The installed ap 1.2.4 help was
checked for the documented connection, doctor, shell, config and product
commands. The existing local `$ap` matched the main source before replacement;
its prior copy is retained at `/Users/justn/.codex/ap-skill-backup.Xtn6Et`.
The three new names appeared in the next Codex turn's available-skill catalog.
The later 1.2.5 package/release is recorded above. Task-level behavior of
each new skill has not been exercised separately.

## Connection lock stability — 2026-09-30

- [x] [AP-09/connection-lock] Retry brief project-connection lock contention without weakening receipt/content checks.
- [x] [AP-19/connection-hotfix-release] Verify and publish a separate 1.2.4 patch release, leaving 1.2.3 tag/assets immutable.

Latest 1.2.3 documentation-only CI repeated a transient connection-lock busy failure
seen in earlier release CI. The immutable source/tag and Homebrew CI passed; this
follow-up improves the actual CLI setup/remove path rather than only rerunning CI.
Local gate: 132 Rust tests, fmt/Clippy, release build, tmux/PTY and presentation
scenarios, source skill validation, exact archive install/uninstall and byte-identical
repackaging passed. Codex/Claude/OpenCode lock fixtures each waited for and recovered
from a short lock; existing receipt, content and symlink checks remained. SHA-256:
1261c3a31a1781bb40e70d6d50c41f4486d2d5861719134d59b4c5ae95e23106.
Published release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.4.
Tag/source 8173d7e86ac9d90afe1cf29432ae5c81c177f4f5 passed Rust CI
(source run 36607607178; tag run 36608077826). Homebrew formula
6484740d754993177841c0e2862e5398a14b4e35 passed install CI 36608169327
and Rust CI 36608169336. Downloaded archive matched the SHA-256 above.
This Mac upgraded via brew to 1.2.4; brew test/audit and installed doctor --strict
passed. Installed binary matched the tested release binary. The global ap skill was
backed up/validated against its prior-source baseline, and managed zsh setup stayed
current without rc changes. Prior managed binary versions and live user panes were
retained. The 1.2.3 release note now points to this connection-lock fix.

## Nested passive-project isolation — 2026-09-30

- [x] [AP-06/passive-boundary] Keep a manifest-free child project separate from its ancestor product plan, including when its own connection is removed.
- [x] [AP-09/passive-native-check] Verify isolated project projection and live installed Claude/OpenCode native plan/resume paths.
- [x] [AP-19/passive-hotfix-release] Publish and install a checked patch release without changing the existing 1.2.2 tag/assets.

Readiness check reproduced a 1.2.2 fault in an isolated nested fixture: follow attached
the child plan to its ancestor's product and wrote ancestor progress. No user data was
changed by that reproduction. The fix records an explicit passive root, respects owned
connection and nested Git boundaries, and continues to select a declaration at the
project's own root first. Prior product state is preserved; it is not silently rewritten.
Target a new 1.2.3 release after checks, keeping v1.2.2 immutable.
Published hotfix: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.3.
Tag/source e0a0e5f19154d2677015ee418c5cf446f6e63a5f passed Rust CI
(branch run 36605833772; tag run 36606160641). Homebrew formula
f826286b14f3e63026a81d7793d73f42bbf3382f passed install CI 36606223342
and Rust CI 36606223423. Downloaded archive matched SHA-256
da1dabcfa3ce7509e4dfd1597739dad63556f179f44b9089beea246cd3d320e6.
Local brew upgrade/test/audit and exact installed binary match passed; the ap skill was
backed up/upgraded and validated. Existing managed zsh setup stayed current, prior
managed binary version was retained, and running user panes/credentials were untouched.
The v1.2.2 release note discloses its nested-project issue and links this hotfix.
Local candidate verified: 131 Rust tests, fmt/Clippy, release build, PTY, skill validation,
exact archive installation/removal and byte-identical repackaging. Archive SHA-256:
da1dabcfa3ce7509e4dfd1597739dad63556f179f44b9089beea246cd3d320e6.
Real Claude Code 2.1.284 (haiku native tasks) and OpenCode 1.18.30
(opencode-go/glm-5.3-flash native todos) each completed 0/2 → 1/2 → 2/2 with
stable IDs and new-process resume in manifest-free private fixtures. Their source
development build was tested before the 1.2.3 version bump. A Codex review found
concurrent first-hook marker publication could drop an event; the idempotent verified
marker fix and parallel regression passed. Previously cached product state is not
automatically rewritten. The 1.2.2 GitHub release notes now disclose that issue.

## Diagnostic completion and release — 2026-09-30

- [x] [AP-09/doctor-complete] Extend read-only doctor checks for connections, shell setup and exact terminal slots; preserve default exit behavior and add opt-in strict checking.
- [x] [AP-06/shell-upgrade-status] Report current/outdated/absent/partial/conflict shell setup and a safe next action without altering user configuration.
- [x] [AP-19/refinement-release] Verify, publish and install the complete refinement package with matching GitHub/Homebrew assets.

The user requested both previously deferred diagnostics and publication. Continue in
the existing skill/CLI task worktree, preserve its prior changes, and include the earlier
manifest-free/opt-out/preview improvements. Target the next patch release, 1.2.2.
Local candidate verified: 129 Rust tests, fmt/Clippy, release build, release-binary
tmux/PTY, skill validation and exact archive install/uninstall. Repacking was byte-identical.
SHA-256: a26b942618b62ea4ff3da97b64155d2a7649abdf50c58afc5a826b66505b1e43.
Independent Astra review found two diagnostic misses (hidden non-UTF8 managed bash file,
invalid roadmap without cached state); both were reproduced, fixed and covered by tests.
Published release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.2.
Tag/source commit 0822dd5324e563f0c1f2fefba9e6c39bac175a7b passed Rust CI
(run 36599817895 and tag run 36600215040). Homebrew formula commit
691988cec8475c91377fbb729934387cbd6651e2 passed install CI (36600288216).
Its Rust CI run 36600288191 passed on full rerun: the first attempt hit transient
connection-lock busy (os error 35) in a concurrent test; neither tag nor source changed.
Published download matched the local archive hash above. Homebrew upgraded this Mac to
1.2.2; brew test/audit and installed doctor --agent codex --shell zsh --strict passed.
Installed binary matched the tested release binary. Managed zsh setup remained current,
global ap skill updated from the exact prior source with private backup and validation,
and live user panes/native credentials were left intact. Native producer tests for the
new manifest-free path and human acceptance were not separately performed.

## Skill and CLI refinement — 2026-09-29

- [x] [AP-06/skill-cli-audit] Review skill/CLI contract gaps with user-requested Astra.
- [x] [AP-09/skill-cli-contract] Verify the findings and implement a small evidence-backed first improvement set.
- [x] [AP-19/skill-cli-docs] Align skill, command help and tests with the verified implementation.

Base: bd5a875148eb746a42222821731ac597925fe9da (published 1.2.1 follow-up).
Workspace: /Users/justn/dev/agent-progress-skill-cli. Astra uses the native Codex subagent
surface, gpt-6-astra at medium effort, for the initial review and a bounded connection
state helper implementation. Root owns CLI/bridge integration, reproduction, tests and
skill alignment. Global setup changes and a new publication are not delegated. Product choices that
cannot be resolved from source evidence remain explicit; routine verified fixes can proceed.

Reproduced on installed 1.2.1 in isolated fixtures: all three preview commands fail without
a manifest, and all three launchers overwrite AP_AUTO_OPEN=0 with 1. The first implementation
set makes product mapping optional, retains explicit opt-out, and adds read-only connection
ownership state to preview. Existing public JSON fields and protected apply/remove remain.
Final local verification: 118 Rust tests, fmt/Clippy, release build, real tmux/PTY fixture
and source skill validation passed. Astra's independent review found a broken-manifest
auto-discovery fallback; it was reproduced and fixed with a regression. Explicit root
matching prevents declaration links from writing another product's state. Native settings
and global installed 1.2.1/skill were not changed. Work is local/unpublished; doctor and
shell-status expansion are the next candidates, not part of this completed first set.

## 1.2.1 release — 2026-09-29

- [x] [AP-19/portable-package] Version and verify the exact 1.2.1 archive and installation.
- [x] [AP-19/portable-publish] Publish the matching source commit, tag, assets and Homebrew formula.
- [x] [AP-19/portable-installed] Verify published checksums, CI and the installed 1.2.1 binary.

Exact 1.2.1 archive installation/removal and its public shell/compatibility/real-tmux
fixture paths passed. Repacking was byte-identical. SHA-256:
d4fedf795f36f6fd78c3e367dcc00caf2c3a1df51832b16becfeb2160a4588ef.
Release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.1.
Tag/source: 8e0b86ee1c9c4baeecec7e25eb7e5c965ddde99c (Rust CI passed).
Homebrew formula: 6005867344044e16d3f5bbfdc9e046900ff69743 (install CI passed).
Published download matched the local archive hash. Homebrew upgraded this Mac to 1.2.1;
brew test/audit and installed CLI/RPC checks passed. The installed binary matched the
release binary. Existing managed zsh setup was backed up/upgraded, and installed ap
skill updates passed validation after checking against the prior source baseline.
Active shells need a new shell or explicit rc reload; running user panes were retained.

## Portable connections — 2026-09-29

- [x] [AP-06/shell-portable] Add bash/fish preview/install/status/remove without replacing user definitions or executables.
- [x] [AP-06/terminal-portable] Automatically show the exact native session outside Herdr using optional tmux.
- [x] [AP-09/compatibility] Detect version changes and check native connection/plan contracts, separating live and fixture evidence.
- [x] [AP-19/portable-verification] Verify real shells/terminal lifecycle and update usage documentation.

Scope: repository changes and isolated verification. Existing global zsh configuration
and user panes are preserved. No additional OS packaging or Apple signing is included.
Implementation is released as 1.2.1. Real zsh/bash/fish, tmux/PTY fixtures, native
markdown producers and review fixes are recorded in verification/portable.md.
Final local verification: 104 Rust tests, fmt/Clippy, release build, release-binary
tmux/PTY and presentation scenarios passed. Native CLI/RPC checks passed on all three
installed agents. Source skill metadata passed validation. Review findings on native
exit status, slot cleanup, Bash login precedence and unobservable Codex flags were fixed
and covered by regression scenarios. The release checklist above tracks public/installed state.

## Shell integration — 2026-09-29

- [x] [AP-06/shell] Implement optional zsh install/status/remove with preservation and backup.
- [x] [AP-09/shell] Verify routing, quoting, native escape, non-interactive passthrough and safe removal in real zsh.
- [x] [AP-19/shell] Release/install the verified shell integration package.

The user's rc was backed up and updated after isolated tests. No original Codex executable
was replaced. Existing sessions need a new shell or explicit rc reload. Native project
trust is not bypassed. Tests cover edited blocks, symlinks, exact original bytes and
post-install user edits; the public command works without a plan file.
Actual shell spelling `codex resume` opened the correct native Codex 0.157.1 session
through server 0.158.0 and automatically opened the observer; focus and 28/12 heights
were retained. 93 tests passed. Non-interactive command bypass was added after review.
Tag commit 552283697849bee045c01367134b7ee7b0d00916 passed CI.
Release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.0
SHA-256: 657bc6f1b941fd14d61ffb584fef6f0b6e01e0ab2cff6efa2842690289cb449d.

## Native connection follow-up — 2026-09-29

- [x] [AP-09/native-recheck] Recheck installed Codex, Claude Code and OpenCode with actual native inputs.
- [x] [AP-06/daemon] Verify the launcher connection fix with shared-server clients, new threads and resume.
- [x] [AP-19/connection-release] Verify the final connection fix and update support documentation and release assets.

Claude Code 2.1.274 and OpenCode 1.18.30 passed native task/todo updates, restricted fixture
file work and new-process resume against ap 1.1.0. Codex 0.157.1 passed native Plan/report,
goal lifecycle, cross-session IDs and app-server restart persistence. This is fresh native
adapter evidence, not proof that every installation or future harness version is supported.

Shared-server frontends reproduce missing pane/session identity in an isolated Herdr
fixture. Hook inputs have session/transcript data but no client PID or pane identity;
the inherited pane environment is insufficient. Herdr client-side identity delivery is
implemented in agent-progress itself; Herdr source changes were not needed for the
launcher route. Plain shared `codex` startup outside that route is not claimed fixed.
Final native lifecycle checks passed: two distinct client sessions in one project;
cross-directory resume; new thread reusing the observer; the other source unchanged;
focus and 28/12 pane heights retained. 89 Rust tests, fmt/Clippy, PTY checks and exact
archive install/removal passed; update/rollback also passed on the prior same-version
candidate. Final tag commit 78130cd9a706b05f135d93635af5b4de72640672 passed GitHub CI.
Release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.1.1
Archive SHA-256: 18fad7f074fa1bb615dd78028401c658572aed9ca99d8ff56fba0c316580796d.
Apple signing/notarization remain excluded.

Agent skill follow-up completed: skills/ap provides the $ap entrypoint and focused
connection, theme and recovery references. Installed locally in the Codex skills
directory; both source and installed copies passed skill-creator validation.
Reference links and standalone config/file commands passed against ap 1.1.0 in an
isolated temporary project. No active user panes or native agent settings were changed.

Homebrew follow-up completed: Formula/agent-progress.rb is published as a custom tap.
Real GitHub download/install/reinstall and brew test passed on macOS arm64; brew style
and audit passed. The Homebrew install CI passed for bd3afcdf321703e17ebe92d929b02074a3593865.
Upgrade/uninstall commands are documented. Existing v1.1.0 assets and checksum remain unchanged.

Goal: publish the 1.1.0 source and unsigned macOS arm64 package to GitHub.

- [x] [AP-16/theme-1.1.0] Implement YAML, presets, brightness and state colors.
- [x] [AP-19/privacy-1.1.0] Exclude credentials, session records and local captures.
- [x] [AP-19/verify-1.1.0] Verify source tests, TUI and exact installed package.
- [x] [AP-19/publish-1.1.0] Publish the matching commit, tag and release assets.

Release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.1.0
Tag commit: 1db02e38e8cf7af8d65cc301b0121c879499b8ef.
83 Rust tests, fmt, Clippy, PTY checks and exact archive installation passed locally.
GitHub CI passed for the tag commit. Repacking produced identical archive bytes.
Archive SHA-256: 171fbef452771012609a8cf54a215bccedc7a25e3149ba21e1f89fade8c3cac2.

Original acceptance criteria remain in product-completion-plan.md. A release substep
does not independently certify the full parent AP item.
