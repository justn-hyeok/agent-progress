# Portable follow-up — 2026-09-29

Target: feat/portable-progress in /Users/justn/dev/agent-progress-portable.
Implementation evidence below remains separate from publication and human acceptance.

- Real zsh, macOS bash and fish 4.9.3: exact argument preservation (quotes, spaces,
  empty arguments and newlines), native escape, existing definitions, backups,
  removal and legacy zsh block upgrade. Default bash setup covers both login/non-login
  rc paths without masking existing login files; default fish directory creation is tested.
  Global user shell files are unchanged.
- Real tmux 3.6a/PTY with fixture agents: concurrent session separation, automatic
  progress, exact source ownership, source focus, resize, termination and native exit
  status. The Codex fixture uses actual Unix/WebSocket forwarding through the production
  connection observer. A separate existing-tmux scenario preserves an unrelated window.
  These are fixture native producers, not a live authenticated interactive pane run.
  Invalid presentation settings do not fail the native hook. Codex override/embedded
  and non-Unix remote modes that bypass observation also bypass automatic pane creation.
- Authenticated native markdown producer through the public compatibility entrypoint:
  Codex CLI 0.157.1 and Claude Code 2.1.274 passed; OpenCode 1.18.30 passed with the explicit
  installed model opencode-go/glm-5.3-flash. Its default-model attempt failed, so do not
  generalize success to every configured provider. Temporary test cwd/settings were isolated.
  Native TaskCreate/todos and full pane lifecycle are not certified by --live.
- Automatic version cache: version changes trigger renewed CLI/RPC checks; a missing
  required interface warns while native execution continues. Failed checks are retried.
  Provider stderr is private and not copied into the report/cache.

Commands: HERDR_ENV=0 cargo test --locked; cargo fmt --check;
cargo clippy --locked --all-targets -- -D warnings;
python3 scripts/pty_terminal.py target/debug/ap;
ap compatibility --agent NAME --live [--model MODEL].

Final result: 104 Rust tests, fmt/Clippy, release build, both release-binary PTY scripts
and source skill validation passed. Local CLI/RPC checks passed on the three installed
agents. Codex review findings on existing-tmux exit status/cleanup, Bash login precedence
and Codex observation bypass were fixed and checked through regressions.

1.2.1 package: exact archive extracted/installed into a temporary prefix, all member
checksums checked, binary hash matched the release build, three shell install/remove
paths and local native interfaces passed, and the installed binary passed tmux/PTY
fixtures. Uninstall retained unrelated files and plan data. Repacking was byte-identical.
Archive SHA-256: d4fedf795f36f6fd78c3e367dcc00caf2c3a1df51832b16becfeb2160a4588ef.
Authenticated markdown producer checks above used the portable development build
before the 1.2.1 version bump; the archive check is separately labeled here.

Publication: v1.2.1 targets 8e0b86ee1c9c4baeecec7e25eb7e5c965ddde99c; Rust CI passed
at https://github.com/justn-hyeok/agent-progress/actions/runs/36550680133.
The downloaded public archive matched the hash above. Homebrew formula commit
6005867344044e16d3f5bbfdc9e046900ff69743 passed install CI at
https://github.com/justn-hyeok/agent-progress/actions/runs/36551086038.
Local brew upgrade/test/audit and installed CLI/RPC checks passed. The installed binary
matched the packaged release build. Managed zsh and installed ap skill updates preserved
unrelated content and backups; neither active user panes nor native credentials were changed.

No Apple signing/notarization or other OS certification is included. tmux is optional
and needed only for automatic display outside Herdr. Native project trust is preserved.
