# Current release plan

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
