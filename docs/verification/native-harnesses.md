# Native harness verification — 2026-09-29

These checks used installed native harnesses and a task-worktree release build of
agent-progress 1.1.0. Source, installed package and published archive evidence are
separate; this run did not replace the immutable v1.1.0 release archive.

| Harness | Version | Model | Actual checks | Result |
| --- | --- | --- | --- | --- |
| Codex | 0.157.1 | gpt-6-sol, low | Native Plan → execution report; goal changes; two sessions; stable task IDs; stale reader preservation; app-server restart | Passed |
| Claude Code | 2.1.274 | haiku | Native TaskCreate/TaskUpdate; restricted fixture file writes; same session resumed in a new process; stable IDs | Passed |
| OpenCode | 1.18.30 | opencode-go/glm-5.3-flash | Native todos; restricted fixture file writes; same session resumed in a new process; stable IDs | Passed |

Claude and OpenCode progressed from zero to one to two completed tasks. Exact fixture
file bytes and stable task IDs were asserted. Codex progressed 0/2 → 1/2 → 1/2 in a
second session → 2/2; old readers did not regress product state. The producer models
were QA inputs, not delegated production implementation.

Reproduce with a release build from the target worktree:

```sh
cargo build --locked --release
AP_BINARY="$PWD/target/release/ap" HERDR_ENV=0 python3 scripts/native_contract.py
HERDR_ENV=0 python3 scripts/agent_contract.py claude --native-tasks --artifact --binary target/release/ap
HERDR_ENV=0 python3 scripts/agent_contract.py opencode --native-tasks --artifact --model opencode-go/glm-5.3-flash --binary target/release/ap
```

These commands make real model requests and create private fixture data. Model access
and installed versions must be checked again before a later run. No global agent
configuration changes are required. Raw producer output and session/authentication
files are retained locally and excluded from Git.

## Shared-server finding

Two Codex 0.157.1 frontends connected to the same already-running native server in a
separate Herdr test session. Both used the same fixture project and had distinct native
thread IDs and plans. `ap follow --pane SOURCE --once` failed for both with
`Codex session identity unavailable`. This is a reproduced source-discovery issue,
not a failed native Plan parser or storage contract.

Lifecycle hook input contains session and transcript identity but no originating pane
or frontend PID. The server can retain an earlier client's environment. Resolving the
window from that environment is therefore insufficient.

## 1.1.1 connection change

The Codex launcher observes its frontend RPC connection while retaining the shared
server. It correlates start/resume/fork replies with that frontend's requests, validates
PID ancestry and transcript identity, and retains a current-selection marker. Unrelated
reads, late replies and stale markers do not certify ownership. Raw HTTP/WebSocket bytes
are forwarded unchanged; conversation content is not stored. The native server handles
the protocol, authentication and agent execution. Herdr code and global agent settings
were not changed.

Launcher coverage uses Codex CLI 0.157.1 and the installed native server 0.158.0 with
two clients in one fixture project. Both clients auto-open separate observers. Final
new-thread/resume checks passed in 1.1.1: cross-directory resume, a new thread reusing
the same observer terminal, the other source unchanged, and focus/28+12 heights retained.
Ephemeral title-generator threads were excluded after reproducing an incorrect selection;
the regression is covered by a dedicated test. Native input/output remained functional.
This validates `ap launch --agent codex`; a shared frontend started directly as `codex`
without this observer connection has not been made automatically identifiable here.

Apple Developer signing/notarization and other OS/architectures remain outside this run.
