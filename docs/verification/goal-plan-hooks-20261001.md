# Goal/plan checklist and project-hook repair — 2026-10-01

Base: `145b9f18e77c5bc17d6f8bbedf5ad144b1a76bf6` (published 1.2.8).
Patch branch: `fix/goal-plan-hooks-20261001`. The package version is unchanged;
this development build is not the published 1.2.8 artifact.

## Change

- Claude/OpenCode explicit Markdown checkpoints retain the parsed goal rather
  than dropping it when normalizing checklist rows. A goal-only change is now
  distinct from a duplicate checkpoint.
- A product-mode pane retains its product acceptance metric and additionally
  shows the agent's session goal and actual session checklist/counts.
- Codex shared-server hooks that cannot prove source ancestry keep a pending
  identity diagnostic, publish no pane binding, and return normal hook JSON.
  This does not claim an automatic connection for unidentifiable frontends.

## Evidence

Three new regression scenarios failed against the original source and passed
after the patch:

1. Explicit goal/plan preservation through both native bridge adapters,
   same-goal state updates, goal-only changes, deduplication and new-process
   replay; unrelated prose is excluded from the stored checkpoint.
2. The rendered product-mode pane exposes the native goal and all three
   session items with 1/3 reported completion while its separate roadmap
   remains 0/2. Rendering leaves the snapshot unchanged.
3. The real hook CLI against an isolated Herdr contract returns exit 0 and
   valid `continue` JSON for an unowned Codex process. No pane binding is
   written; `registered_pane:false`, `source_identity_unverified:true` and
   the pending diagnostic remain in local status.

Commands passed in the task worktree:

```sh
HERDR_ENV=0 cargo test --locked --all-targets
cargo fmt --check
HERDR_ENV=0 cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
HERDR_ENV=0 python3 scripts/pty_presentation.py target/release/ap
HERDR_ENV=0 python3 scripts/pty_product.py target/release/ap
git diff --check
```

Both PTY scripts verified terminal restoration. Product PTY also retained
50% overall acceptance while a temporary subplan was 100%.

## Local application and limits

The actual project hook definition referenced an old 1.1.0 development
executable in `target/release/ap`, rather than the Homebrew executable.
Its status file reported the unowned-pane error. With the same current
daemon-session input, the original executable returned exit 1 and the patch
returned exit 0/continue JSON.

The old executable was backed up byte-for-byte and the development path was
atomically updated to the tested build. All three configured command probes
(SessionStart, UserPromptSubmit and Stop) returned exit 0 and valid JSON,
with the unverified-source diagnostic retained and no fabricated binding.
The applied executable also passed isolated Claude/OpenCode goal/plan state
updates, goal-only changes and same-session replay in new processes.

Hook definitions, native trust and Homebrew were not changed. Raw sessions,
credentials, captures and machine-specific receipts remain private. These
manual command and fixture checks are not authenticated live producer or
human-acceptance evidence. Subsequent UI hook events and full native work
flows remain to be verified. This patch does not resolve every finding of
the repository-wide review and does not certify the complete AP-05 item.
