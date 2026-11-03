---
name: ap
description: Record your working plan with the ap CLI so the user sees progress in a pane below yours. Use for any multi-step task (implementation, debugging, review, research) as soon as you know the steps; skip only for one-shot questions or trivial single actions.
---

# ap — agent-progress

Keep your real plan in `ap` while you work, without being asked. Recording does
not open anything by itself: when the user wants to watch, `ap open` shows a
progress pane under yours (Herdr or tmux) that keeps following your plan. You
never pass IDs, revisions or session names: `ap` keys the plan to your pane.

```sh
ap goal "로그인 버그 수정"
ap add "원인 재현" "토큰 갱신 수정" "회귀 테스트"   # numbered 1, 2, 3 on an empty plan
ap start 1
ap done 1
ap block 2 "스테이징 키 필요" --needs user
ap cancel 3 "범위 밖"
ap status
```

- Items are selected by the number printed in the output, or by exact title.
  Numbers never change, even after `ap rm`.
- Every command prints the current plan; read numbers from it. The plan may
  already have items, so never assume a new item is number 1.
- Record when you start, finish, or get blocked on a meaningful step. Do not
  add an item per tool call. Add newly discovered work with `ap add`; the
  ratio may drop, which is expected.
- `ap block` needs a reason. Use `--needs user` when the user must act.
- `ap cancel` only for work that was explicitly dropped. Cancelled items leave
  the denominator.
- Done is your report, not verification. Say what was verified in the reply.
- `ap new` archives the plan and starts a fresh one for an unrelated task.
  `ap goal` refuses a different goal while the plan has items: run `ap new`
  first, or `ap goal --edit` when only rewording the same task.
- Plans belong to one Git root. Another worktree has its own plan, so record
  from the folder that holds yours, or start a plan for the worktree.

Subagents share the plan of the pane they run in. If you are a subagent, do not
run `ap`, least of all `ap goal` or `ap new`; report to your parent, which
records the work. If you spawn subagents, record their steps yourself and do
not ask them to use `ap`. A worker in its own pane keeps its own plan as usual.

Run `ap open` only when the user asks to see progress. If it reports that it
cannot confirm the pane (Codex and Cline run commands in a shared daemon), the
plan is still recorded; tell the user the printed `ap --plan … view` command
once and do not guess another pane. Do not reopen a pane the user closed.
