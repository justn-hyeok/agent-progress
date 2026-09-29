# Release handoff

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
