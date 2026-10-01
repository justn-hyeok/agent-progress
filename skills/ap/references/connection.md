# Install and connect

## Installation

On macOS Apple Silicon, when installation is authorized:

```sh
brew tap justn-hyeok/agent-progress https://github.com/justn-hyeok/agent-progress
brew install justn-hyeok/agent-progress/agent-progress
ap --version
```

The release archive is also available at
https://github.com/justn-hyeok/agent-progress/releases. Other platforms are unverified.
The binary has no Apple Developer signing or notarization.

## Project-local connection

With ap 1.2.0, `ap shell install` optionally adds a managed zsh function so typing
`codex` or `codex resume` in a new Herdr shell uses `ap launch`. It backs up the rc,
preserves unrelated content and existing definitions, and offers `ap shell remove`.
This is user-level shell setup: apply only when the request authorizes that installation.
It does not replace native binaries or bypass harness trust. Management/non-interactive
commands use native Codex; `command codex` is the direct escape.
ap 1.2.1 also supports `--shell bash` and `--shell fish`. Default bash installation
covers .bashrc and the existing active login file without hiding .bash_login/.profile;
fish uses XDG_CONFIG_HOME/fish/config.fish. The same --shell
must be used for status/removal. Legacy zsh managed blocks upgrade with exact backups.
After upgrading from 1.2.0 rerun ap shell install for the selected shell to update its block.

Select the actual native harness (`codex`, `claude` or `opencode`). From the requested
project, inspect `ap connect preview --agent NAME`. ap 1.2.2 adds these fields
without removing the prior preview payload:

| connection_status | next_action | Decision |
|---|---|---|
| unmanaged | apply | Apply only when project-local setup is authorized. |
| managed | none | Retain the existing connection; repeated apply is unnecessary. |
| conflict | inspect | Inspect the static diagnostic and local ownership evidence; preserve edits. |

Preview is read-only and does not disclose settings contents. Unrelated user settings
can coexist with a managed connection; edited/missing/duplicate managed blocks or hooks,
an invalid receipt, an unowned plugin, or unsafe paths need inspection. Never automate
remove/apply as conflict recovery. Older 1.2.1 previews lack these fields: absence is not
proof that a connection is unmanaged, and a project declaration error is not a reason
to invent a manifest. Upgrade only when authorized; retain known managed settings.

With ap 1.2.2, an ordinary project needs no ap.project.json for passive
hook connection. When an existing valid manifest is present, Codex additionally installs
the product MCP integration and declared product mapping stays enabled. Invalid manifests
remain errors. `--allow-writes` for product MCP requires that existing declaration.
Hooks retain exact root/session matching and never create a product/goal declaration.
Removal is
`ap connect remove --agent NAME` and preserves private backups.

Codex uses project `.codex/config.toml` and managed AGENTS guidance; Claude uses
`.claude/settings.local.json`; OpenCode uses `.opencode/plugins/agent-progress.js`.
These adapters store explicit plans and session identity, not raw conversations.
Native trust/reload requirements belong to the harness. Configuration alone is not proof
that hooks have run; check actual event reception with `ap doctor` for the known source.

## Read-only diagnostics and shell upgrade

An ordinary manifest-free project nested under another product stays independent in
1.2.3. Its passive-root marker and retained connection receipt keep historical sessions
out of the ancestor's product plan even after disconnect. A declaration at the child's
own root takes precedence. Never remove the marker merely to change a parent percentage.
If 1.2.2 was used in that arrangement, inspect the ancestor's stored history rather than
auto-restoring or silently recertifying affected tasks.

`ap doctor --agent NAME` checks that selected native CLI and its project connection;
other installed harnesses are optional. Without a product declaration passive mode is
normal. A declared product with no cached observations is not an invented goal or failure.
Invalid declarations, corrupt cached product identity, ownership conflicts and unreadable
explicit sources are unhealthy. Default doctor returns its JSON report with exit 0;
`--strict` returns nonzero when unhealthy so scripts can enforce their requested checks.

`ap doctor --shell zsh --rc PATH` checks one selected file without executing it. With no
explicit shell, a supported SHELL selects default user setup; absent automatic setup is
informational. `--rc` alone selects zsh. `ap shell status --shell NAME` reports current,
outdated, absent, partial or conflict with a safe next action and upgrade_required. Only
a recognized legacy block is an upgrade candidate. An edited block stays conflict and
must not be replaced or removed automatically. Default bash handles active and previously
owned login files; user login-file precedence is preserved.

`ap doctor --terminal-slot PATH` inspects that exact per-launch slot (or current
AP_TERMINAL_SLOT). It validates metadata, owner liveness and selected transcript identity;
waiting startup is distinct from stale/conflict. It never scans for a newest slot, reads
focused-pane state, emits terminal request arguments, or repairs native settings.

## Pane source and placement

From the project root, `ap setup` selects the initial progress-pane height.
For scripts or later changes, `ap config set --pane-size 25` saves a 10–50%
height preference. The exact owned Herdr or tmux observer resizes immediately
when the command runs from its source pane; otherwise the next opening uses
the preference. Automatic reconnection preserves a height adjusted directly
in Herdr. The terminal layout engine's minimum height wins when needed.

Inside the current agent's Herdr pane, `ap open` uses the exact source and saved placement.
Connected hooks open the observer after the exact session gains a goal or plan.
An empty session keeps the source pane full-sized. The new default observer height
is 10%; explicitly saved sizes still apply. `auto_open=false` or `AP_AUTO_OPEN=0`
disables automatic opening.

```sh
ap open --position above
ap config set --position below
ap config set --auto-open false
```

The first command changes this opening/move only; the second saves the default. Existing
owned vertical siblings exchange position and restore sizes. Unknown, busy or protected
layouts are retained. A viewer above the source can resolve its lower neighbor.

For a live-verified source, `ap doctor --pane SOURCE` diagnoses ownership, session and
received input. `ap open --pane SOURCE --reconnect` explicitly reconnects a changed native
session or restores a closed owned observer. Never guess SOURCE from focus or the latest log.

Codex shared-server hooks can inherit the wrong client's pane environment. With ap 1.1.1,
`ap launch --agent codex` observes the frontend's actual start/resume/fork responses
and keeps the shared server enabled. It verifies process ownership and the selected
session rather than relying on inherited pane variables. A directly started shared
`codex` frontend without this connection may remain unidentified; do not claim that
a version check alone fixes it. Honor explicit native flags such as `--no-daemon` and
configuration overrides. Do not restart an active user session or change global settings.

No Herdr: `ap follow --rollout /explicit/session.jsonl --once` reads a selected Codex source;
`ap product summary` gives saved product text. File CLI, configuration and local stdio MCP
also work without Herdr. With the portable launcher, interactive `ap launch --agent NAME`
opens a tmux source/progress window automatically. Install tmux only when that setup is
authorized. Each launch has a private slot and exact session identity; never select recent logs.
Codex retains its shared server; Claude/OpenCode need their project-local hooks. Existing tmux
windows and non-interactive commands are preserved. Native trust/authentication still apply.

`ap compatibility` checks local CLI/RPC interfaces without a model call. Launches repeat this
check on version change and retain the result in project-local compatibility metadata. Use
`--agent NAME --live` only when native producer testing is authorized: it makes a short model
request in disposable storage using existing authentication. `--model` can select a model for
one agent. A successful markdown producer check does not certify native TaskCreate/todos or
the complete pane lifecycle. Never print raw provider error logs or credentials.
