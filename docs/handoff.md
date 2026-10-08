# Release handoff

3.3.0 is the latest release (progress pane redesign; see docs/current-plan.md
for evidence). Installed via Homebrew and ~/.local on this Mac.

3.0.0-beta.2 (prerelease, https://github.com/justn-hyeok/agent-progress/releases/tag/v3.0.0-beta.2)
fixes all code-review findings and is installed in ~/.local; see current-plan.md.

3.0.0-alpha.2 (prerelease, https://github.com/justn-hyeok/agent-progress/releases/tag/v3.0.0-alpha.2)
restores list/detail/search/history views and is installed in ~/.local. The
ap skill is installed globally (~/.agents/skills/ap, linked for Claude, Codex,
OpenCode).

3.0.0-alpha.1 is a prerelease at https://github.com/justn-hyeok/agent-progress/releases/tag/v3.0.0-alpha.1,
built from branch feat/minimal-cli (not merged to main). It replaces hooks,
launchers and the v2 record protocol with a minimal CLI (goal/add/start/done/
block) and a Signal-style progress pane. Homebrew still installs 1.2.8. This
Mac runs the alpha from ~/.local (rollback: install.sh --rollback → 2.0.2).
Unverified: tmux, OpenCode,
`ap new` with an open viewer. See the first section of docs/current-plan.md.

1.2.8 was published at https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.8
and installed on this Mac via Homebrew. Repeated native-hook openings no longer reset a
manually adjusted Herdr progress pane to the saved size. The 1.2.7 behavior
was reproduced with an exact-pane CLI fixture; explicit config size changes
remain live. The local package passed authenticated three-agent smoke and a
byte-identical repack (SHA-256
ed52a7b7ee66e87d0cfafd3ea3bb3047291d9ad49664d8968d1953d0419ef2e0).
The published archive matched the local candidate. Formula and Rust CI,
installed binary/skills, `brew test`, strict audit and installed PTY passed.
See the first section of `docs/current-plan.md` for exact verification and
publication evidence. Existing user panes have not been modified.


1.2.7 was published at https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.7
and installed on this Mac via Homebrew. Automatic observers wait for
the exact native session's first goal or plan, and new default pane height is
10%. Explicit saved sizes are retained. Herdr hook and Codex frontend, plus
tmux launch, have been changed. Full local Rust/PTY, fmt, Clippy, source skill
validation and diff checks passed; the Herdr path has a mocked CLI contract,
and Codex goal-only opening has a real tmux/WebSocket fixture. The exact archive
passed authenticated Codex/Claude/OpenCode package smoke and a byte-identical
repack (SHA-256 a1062287d6764fd162e0af947e1f85b9a031f425aeb2acbd3e29a7daae890cd4).
Exact source/tag, CI, archive, Formula and local installation evidence is in
the first section of `docs/current-plan.md`. The separate Linux 1.3 candidate
is still unpublished.

1.2.6 is published at https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.6.
`ap setup` chooses project-local 10–50%
progress height, and `ap config set --pane-size N` changes it later. Exact
owned Herdr/tmux observers resize immediately from their source pane; another
terminal saves the preference for the next opening. Existing settings retain
30%. Installed Herdr clamped a 2% request to its 10% minimum in a disposable
owned pane. Full local Rust, PTY, skill and exact archive smoke passed; archive
SHA-256 is f52e82a53f8a77766de23039637434596eb818f9221926322a398f31dd905aea.
Source/tag 2cd7e52fa45c95b607a4c4f778d8e26d4dde13e6 and formula
cd60db6894f8094784ce2acf2abf437701f5ea48 passed their GitHub CI.
The published archive matched the local checksum. This Mac upgraded via
Homebrew; installed binary, skills, `brew test/audit`, doctor and first-run
setup passed. Local Codex skills were backed up/upgraded; the prior 1.2.5
Cellar copy remains. The 1.2.5 public release itself is unchanged. See the
first section of `docs/current-plan.md` for exact evidence and limits.

OpenCode QA follow-up: the native fixture now requires a successful
`todowrite` call instead of accepting a text-only answer, asks for an
available file-editing tool, and lets package smoke select its OpenCode
model per run. A fresh direct OpenCode 1.18.30 / `gpt-6-luna` run and the
full installed 1.2.5 archive smoke both passed without corrective prompts.
The archive smoke also passed Codex/Claude native paths, upgrade, rollback
and uninstall. The 1.2.5 binary and public assets are unchanged; see the
first section of `docs/current-plan.md` for the evidence boundary.

1.2.5 is published at https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.5.
Its source/tag commit b9e8064d845164662698012464d923cbecfb6415 passed
Rust CI; formula a303efa675b4ec1a1879aac68a843c80c57c7516 passed
Homebrew install and Rust CI. The public archive SHA-256 is
01af4d0d6327aeda74f41aed53e0713c27d80fd98d05a3451abcd8ac0cc2ed68.
All four Codex skills are bundled in the archive and installed by Homebrew
under `share/agent-progress/skills`; local installed files matched source.
Exact package update/rollback, reproducibility, Rust/PTY, `brew test/audit`
and installed doctor passed. Local Codex skills were backed up/upgraded.
OpenCode's first `glm-5.3-flash` producer request timed out. A subsequent
installed OpenCode 1.18.30 / `opencode-go/gpt-6-luna` run verified native
todos 0/2 → 1/2 → 2/2, stable IDs, exact-session new-process resume, and
test-file edits after one corrective prompt for a skipped tool call. This is
live connection evidence, not a claim that every model prompt selects tools
reliably. See the first section of `docs/current-plan.md`.

Skill split: `$ap-connect`, `$ap-theme` and `$ap-recover` are now
separately invokable source skills, with `$ap` retained as a compatible general
entrypoint. README explains all four. The four source and installed folders
passed validation and matched byte-for-byte; local ap 1.2.4 help matched the
documented commands. Installed `$ap` had matched the published main source
before the update and was backed up. The new names appeared in the next Codex
turn's skill catalog. Individual skill
workflows have not been exercised separately. See the first section of
`docs/current-plan.md` for exact local evidence and next status.

1.2.4 is published at https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.4.
Tag/source 8173d7e86ac9d90afe1cf29432ae5c81c177f4f5 passed Rust CI.
Homebrew formula 6484740d754993177841c0e2862e5398a14b4e35 passed install/Rust CI.
Archive SHA-256 is 1261c3a31a1781bb40e70d6d50c41f4486d2d5861719134d59b4c5ae95e23106.
Published assets, exact package installation, local brew upgrade/test/audit, installed binary
hash, doctor --strict and the installed ap skill were verified. Managed zsh setup remained
current without editing rc; live user panes and credentials were retained. The v1.2.3
release note names the intermittent connection lock issue and links this fix.

1.2.4 hotfix candidate bounds project-local connection lock retries at two seconds.
Existing receipt, content and symlink checks remain after lock acquisition. The
documentation-only 1.2.3 CI reproduced a transient macOS lock-busy error that had
also appeared in earlier release CI. A direct three-agent contention regression
passes locally. The full local gate passed with 132 Rust tests, fmt/Clippy,
release-binary tmux/PTY, exact archive install/uninstall and a byte-identical
repack. Archive SHA-256 is
1261c3a31a1781bb40e70d6d50c41f4486d2d5861719134d59b4c5ae95e23106.
Release/installation evidence is pending.

1.2.3 is published at https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.3.
Tag/source e0a0e5f19154d2677015ee418c5cf446f6e63a5f and formula
f826286b14f3e63026a81d7793d73f42bbf3382f passed Rust/Homebrew CI.
Archive SHA-256 is da1dabcfa3ce7509e4dfd1597739dad63556f179f44b9089beea246cd3d320e6.
The public download, exact local installation, installed CLI, managed shell and ap skill
were verified. Live user panes were retained; the old 1.2.2 binary was not cleaned up.
The 1.2.2 release note names the nested-project issue and links 1.2.3.

1.2.3 hotfix candidate: a nested manifest-free project's native session no longer
projects into its ancestor's product state. A private passive boundary is retained
after connector removal, and an explicit declaration at the child root still wins.
The 1.2.2 defect was reproduced in a disposable nested fixture; no user product
state was changed by that test. Prior 1.2.2 state is preserved for evidence and
is not rewritten automatically. Actual Claude 2.1.284 native tasks and OpenCode
1.18.30 native todos progressed 0/2 to 2/2 in manifest-free project fixtures,
including resume in new processes. Release/installation evidence is pending.

1.2.2 is published: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.2.
Tag/source 0822dd5324e563f0c1f2fefba9e6c39bac175a7b passed Rust CI. Archive SHA-256
is a26b942618b62ea4ff3da97b64155d2a7649abdf50c58afc5a826b66505b1e43.
Homebrew formula 691988cec8475c91377fbb729934387cbd6651e2 passed install CI;
its Rust CI passed on rerun after one transient concurrent connection-lock busy error.
The published archive and installed 1.2.2 binary matched the tested build. Local brew
test/audit and installed doctor --strict passed. Global ap skill was backed up/upgraded;
managed zsh setup was current and running user panes/credentials were preserved.

1.2.2 includes the complete skill/CLI refinement: manifest-free passive connection,
AP_AUTO_OPEN=0 preservation, preview ownership status, expanded read-only doctor with
opt-in --strict and exact terminal slots, and shell current/outdated/partial/conflict
classification. Existing product mapping/MCP, user settings and protected apply/remove
remain intact. Verified outcomes are recorded in docs/verification/skill-cli.md.

1.2.1 is published at https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.1.
Tag/source is 8e0b86ee1c9c4baeecec7e25eb7e5c965ddde99c; source Rust CI passed.
Archive SHA-256 is d4fedf795f36f6fd78c3e367dcc00caf2c3a1df51832b16becfeb2160a4588ef.
The matching Homebrew formula and install CI passed at 6005867344044e16d3f5bbfdc9e046900ff69743.
Published assets and the local Homebrew installation were verified. Existing local zsh
setup and installed ap skill were backed up/upgraded; active user shells/panes were preserved.

1.2.1 includes the portable follow-up: bash/fish
managed shell setup, optional tmux automatic display outside Herdr, and compatibility CLI
with automatic version-change interface checks and optional authenticated markdown producer
smoke. Existing zsh users should rerun ap shell install after upgrading the binary to
update the legacy managed block with a backup. README/operations/skill references describe
the supported paths. See verification/portable.md for exact evidence.

1.2.0 adds optional zsh command setup with preview/install/status/remove. After installation
in a new shell, ordinary codex/resume enters the existing launcher inside Herdr; outside
Herdr and for management/non-interactive commands native execution is preserved. The real
shell/native-resume path and automatic observer passed; unrelated rc edits and removal
were tested. Native project trust is preserved. Other shells and direct program invocations
that bypass shell functions are not claimed supported by this setup.

1.1.1 adds a frontend connection observer for the Codex launcher while retaining the
shared native server. Two clients, resume, new-thread reuse and layout preservation
passed live on Codex CLI 0.157.1 / native server 0.158.0. Ephemeral helper threads and
unrelated reads are excluded. Direct shared `codex` startup without the launcher is
not claimed automatically identifiable. Native harness rechecks are in
verification/native-harnesses.md. Herdr source and global agent settings were unchanged.

Version 1.1.0 includes YAML themes, built-in and inherited presets, background brightness
and per-state palettes, in addition to the 1.0.x native adapters, passive pane dashboard,
recoverable storage and unsigned macOS arm64 installer.

The original AP-01 through AP-24 product acceptance was completed locally for 1.0.0.
Reported, automated and human evidence remain distinct. Raw native sessions, local state,
credentials and personal terminal captures are retained locally and excluded from Git.

Herdr is optional for standalone file operations, explicit rollout reading, MCP and
configuration. Pane placement and automatic process/session identification require Herdr.
Apple Developer signing and notarization remain unavailable. Other platforms are unverified.

See operations.md, distribution.md, CHANGELOG.md and the source tests for reproducible
interfaces. Older exploratory design documents describe historical proposals.
