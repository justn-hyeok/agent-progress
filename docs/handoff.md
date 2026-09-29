# Release handoff

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
