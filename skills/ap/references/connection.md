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
commands and non-Herdr shells use native Codex; `command codex` is the direct escape.

Select the actual native harness (`codex`, `claude` or `opencode`). From the requested
project, inspect `ap connect preview --agent NAME`. If a connection is needed and the
request authorizes setup, run `ap connect apply --agent NAME`. Already managed settings
should be retained; conflicting user settings are not overwritten. Removal is
`ap connect remove --agent NAME` and preserves private backups.

Codex uses project `.codex/config.toml` and managed AGENTS guidance; Claude uses
`.claude/settings.local.json`; OpenCode uses `.opencode/plugins/agent-progress.js`.
These adapters store explicit plans and session identity, not raw conversations.
Native trust/reload requirements belong to the harness. Configuration alone is not proof
that hooks have run; check actual event reception with `ap doctor` for the known source.

## Pane source and placement

Inside the current agent's Herdr pane, `ap open` uses the exact source and saved placement.
Working hooks open the observer automatically unless `auto_open=false` or `AP_AUTO_OPEN=0`.

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
also work without Herdr. Neither path creates a pane automatically.
