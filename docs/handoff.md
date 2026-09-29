# Release handoff

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
