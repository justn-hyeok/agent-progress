---
name: ap
description: Show an existing coding-agent goal and plan in an agent-progress pane, or route a general ap request. For focused setup, colors, or recovery, use ap-connect, ap-theme, or ap-recover. Ordinary coding or a generic progress update alone does not require it.
---

# ap — agent-progress

Keep the user's existing goal and plan visible without asking them to register tasks,
copy session UUIDs or maintain a second checklist. The observer reads progress; it does
not supervise agents or prove that reported work passed verification.

## Start from the current project

Check `ap --version` and the relevant command's `--help`; this skill targets 1.2.5.
Check actual preview/doctor fields before assuming an older binary has these contracts.
If the binary is missing, install it when installation is within the request, using
[connection.md](references/connection.md). Do not change global agent settings.

For a progress pane, verify the current agent's Herdr context and exact source ownership.
Run `ap open` from that agent's pane; it opens or reuses the managed observer without
changing focus. Connected hooks normally do this automatically. Use an explicit `--pane`
only when that source is established by live metadata, not the focused or newest pane.
Do not split an extra pane manually if an owned observer already exists.

If connection is absent or fails, read [connection.md](references/connection.md). Resolve
the local cause yourself when authorized. Ask only for a necessary user-owned trust or
authentication action. Do not ask the user to paste IDs that local metadata can supply.
Outside Herdr, `ap launch --agent NAME` can create an automatic tmux progress window in an
interactive terminal when tmux is installed. Exact per-launch identities are required;
Claude/OpenCode still need their project hooks. File mode and explicit rollouts also work.

For setup, use `ap connect preview --agent NAME` as the decision point. When it exposes
`connection_status`, apply only an `unmanaged` connection within the authorized setup;
retain `managed` settings and inspect a `conflict` without remove/reapply loops. A product
manifest is optional for passive observation; its presence adds the declared product mapping.
`AP_AUTO_OPEN=0` keeps automatic opening disabled through the native launcher and hooks.

For an ordinary project nested under another product, keep the child's connection and
session history separate. If 1.2.2 was used there, inspect the ancestor's product history
instead of changing its completion claims automatically.

For diagnosis use `ap doctor` with the known agent/source. Add `--strict` only when an
unhealthy requested check must produce a failing exit code. `--shell NAME --rc PATH`
checks explicitly selected setup; `ap shell status` distinguishes known legacy setup
(`outdated`, upgrade_required) from user-edited `conflict`. Upgrade only the former within
authorized setup. Diagnosis never applies repairs or picks another session automatically.

## Maintain the existing plan

Use the harness's native plan/task tool when available. For a harness adapter that reads
explicit assistant reports, publish the real plan at meaningful checkpoints:

```markdown
### 진행 계획
목표: 사용자와 합의한 현재 목표
- [x] 이미 끝낸 작업
- [>] 지금 진행 중인 작업
- [ ] 다음 작업
```

Keep unchanged item wording stable. If `ap.project.json` defines roadmap IDs, link the
existing IDs in the rows. A child such as `[AP-05/native]` cannot certify the entire
`[AP-05]` acceptance item. Do not invent a product manifest, new native goal or reduced
scope merely to make a progress bar appear complete.

Distinguish agent reports, observed process activity, automated checks and human evidence.
Checklist completion is neither an ETA nor independent product acceptance. Missing items
are not silently cancelled. Never mark unverified work complete to improve the percentage.

## Task-specific actions

- Connection, automatic opening, placement and exact-session diagnosis: use
  `$ap-connect` when installed; this standalone skill retains
  [connection.md](references/connection.md) for existing `$ap` installations.
- Presets, brightness, YAML and hot reload: use `$ap-theme` when installed;
  standalone details remain in [themes.md](references/themes.md).
- Resume, evidence, backup/restore and portable state: use `$ap-recover` when
  installed; standalone details remain in [recovery.md](references/recovery.md).

Apply only the requested changes. Preserve existing plans, project data and user settings.
Do not write raw transcripts, credentials or unrelated conversation into progress evidence.
If source identity remains ambiguous, stop that connection and report the specific cause;
do not attach to another session by recency.
