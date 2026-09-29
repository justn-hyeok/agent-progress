# Connection lock stability — 2026-09-30

Source base: published v1.2.3. The documentation-only CI run
https://github.com/justn-hyeok/agent-progress/actions/runs/36606634636
failed in `optional_integration_preserves_user_text_and_remove_is_narrow` when
an immediate subsequent project-local lock returned os error 35 (`settings busy`).
Source/tag/formula CI on the immutable 1.2.3 artifact had passed, but the repeated
transient was a real CLI setup weakness rather than a reason to move that tag.

The Codex and Claude/OpenCode connection setup/remove paths now retry only
WouldBlock lock attempts, for at most two seconds. They retain the existing
receipt ownership, exact-byte comparison, backups and symlink protections after
acquisition; persistent contention still fails without replacing user settings.
One isolated regression holds and releases each agent's exact lock before apply.

Local tests, archived package, public source, CI and installed evidence are tracked
separately. Native model requests and live session QA are not repeated by this lock fix.

Local gate: 132 Rust tests, fmt/Clippy, release build, tmux/PTY and presentation
scenarios, source skill validation and exact archive installation/removal passed.
Repacking was byte-identical. Archive SHA-256:
1261c3a31a1781bb40e70d6d50c41f4486d2d5861719134d59b4c5ae95e23106.
The packaged CLI retained v1.2.3 child/ancestor isolation, current shell states,
strict doctor behavior and narrow uninstall. Public CI/installation status follows
the exact 1.2.4 tag and Homebrew formula, without modifying older release tags.
