# Nested passive connection hotfix — 2026-09-30

Target: feat/skill-cli-upgrade, based on published v1.2.2.

Installed 1.2.2 reproduced a cross-project projection in a disposable fixture:
a plain child under a declared parent connected and emitted a plan; `ap follow --once`
attached the parent's product and created parent cached progress. No real user project
data was changed. The matching 1.2.3 source fixture returned a session-only view and
left the parent state absent. Regression tests cover Codex/Claude/OpenCode fixture
input, post-disconnect history, unrelated parent subdirectories and simultaneous
first hooks. An altered or unsafe boundary fails closed instead of choosing a parent.

Real native producer QA against the hotfix source (compiled before version bump):

| Harness | Version / model | Observed result |
| --- | --- | --- |
| Claude Code | 2.1.284 / haiku | Native TaskCreate/TaskUpdate, 0/2 → 1/2 → 2/2, stable IDs, resume in new process |
| OpenCode | 1.18.30 / opencode-go/glm-5.3-flash | Native todos, 0/2 → 1/2 → 2/2, stable IDs, resume in new process |

These model calls were restricted to disposable projects, without product declarations,
file writes or subagents. Raw producer output and session records are private and ignored
by Git. Codex's native interactive client was not separately rerun; its new boundary
path has deterministic session fixtures and the existing frontend transport tests.

Final local gate: 131 Rust tests, fmt/Clippy, release build, real tmux/PTY and presentation
scenarios, source skill validation, exact archive installation/removal, and byte-identical
repackaging. The installed-package fixture proved a child could not create/modify parent
product state and remained separate after disconnect. Archive SHA-256:
da1dabcfa3ce7509e4dfd1597739dad63556f179f44b9089beea246cd3d320e6.

Existing parent state from 1.2.2 is preserved, not automatically restored; the old
release note names the issue. Apple Developer signing and other OS/architectures are
outside this verified release scope. Publication/CI/installation evidence is added
after the matching tag and package are live.
