---
name: ap-connect
description: Set up or diagnose an agent-progress connection for Codex, Claude Code, or OpenCode, including automatic progress-pane opening and exact session ownership. Use for connection, shell integration, or pane placement; use ap-theme for colors.
---

# ap-connect — native agent connection

Connect the current project's existing plan to a passive progress pane. Never ask
the user to register a task, copy a session ID, or maintain a second checklist.
The pane reflects reports; it does not supervise work or certify completion.

Check `ap --version` and relevant `--help` first. These instructions describe
ap 1.2.4; check preview and doctor fields before relying on them. If the binary
is missing, install it only when installation is within the request. On macOS
Apple Silicon, the published Homebrew path is:

```sh
brew tap justn-hyeok/agent-progress https://github.com/justn-hyeok/agent-progress
brew install justn-hyeok/agent-progress/agent-progress
```

## Project connection

Select the actual harness: `codex`, `claude`, or `opencode`. From that project's
root run `ap connect preview --agent NAME`. A product manifest is optional for
passive observation; an existing valid one adds declared product mapping.

| `connection_status` | Action |
|---|---|
| `unmanaged` | Apply with `ap connect apply --agent NAME` when project setup is authorized. |
| `managed` | Keep it; repeated apply is unnecessary. |
| `conflict` | Inspect ownership and local diagnostics; preserve edited settings. |

Do not remove and reapply a conflicting connection automatically. An older
binary may omit `connection_status`; absence does not mean `unmanaged`.
Codex uses project `.codex/config.toml` and managed AGENTS guidance, Claude
uses `.claude/settings.local.json`, and OpenCode uses
`.opencode/plugins/agent-progress.js`. Preserve unrelated settings and native
trust requirements. Do not change global agent settings. `AP_AUTO_OPEN=0`
remains an explicit opt-out of automatic opening.

For ordinary nested projects, keep the child's passive history separate from
the ancestor product. If 1.2.2 was used there, inspect the ancestor's history;
never silently change its completion claims.

## Open and diagnose

From the current agent's Herdr pane, `ap open` opens or reuses its managed
observer without changing focus. Connected hooks normally open it. Use an
explicit `--pane` only after verifying that source from live metadata; never
pick the focused pane or newest log as a substitute. Do not create a second
observer manually. For an established source, `ap doctor --pane SOURCE` checks
ownership/session/input; `ap open --pane SOURCE --reconnect` can explicitly
reattach a changed native session. Preserve busy or unowned panes.

`ap open --position above` moves this opening; `ap config set --position below`
saves future placement. An owned vertical sibling swaps position and sizes.
Saving the preference alone does not move the live pane. Color and brightness
changes belong to `$ap-theme`.

Use `ap doctor --agent NAME` for the known agent/source. Doctor's default JSON
exit is 0 even when it reports unhealthy checks; add `--strict` when a failing
exit is required. `ap doctor --shell NAME --rc PATH` inspects an explicit shell
file, and `ap doctor --terminal-slot PATH` inspects one exact launch slot.
Neither finds a newest session or repairs configuration. A successful
`ap compatibility` CLI/RPC check does not prove a native plan was produced;
`--live` makes a model request and needs authorization for that test. Do not
print raw provider errors or credentials.

Optional `ap shell install --shell zsh|bash|fish` sets up native commands such
as `codex` and `codex resume` to route through `ap launch` in a new interactive
shell. Preview/status before installing; install only when shell setup is
within the request. Use `ap shell status --shell NAME` to distinguish current,
outdated, absent, partial and conflict. Upgrade recognized legacy setup only;
preserve edited blocks. `ap shell remove` is an explicit removal operation.
The native binary is not replaced and `command codex` bypasses the function.

With a Codex shared server, the `ap launch --agent codex` frontend observer
identifies its own start/resume/fork. A direct shared `codex` frontend that
bypasses it may lack exact pane identity. Do not guess by process recency or
restart a user's active session. Outside Herdr, interactive `ap launch --agent
NAME` can create an optional tmux progress window with exact per-launch
identity; Claude and OpenCode also need their project hooks. Explicit
`ap follow --rollout /path/to/session.jsonl --once` and file/product commands
work without Herdr.

Keep agent-reported progress, process activity, automated checks and human
verification distinct. If source identity is ambiguous, report that exact
cause instead of attaching to another session.
