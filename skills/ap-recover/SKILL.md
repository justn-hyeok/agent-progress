---
name: ap-recover
description: Resume or recover agent-progress product state, inspect saved progress and evidence, or safely back up, restore, export, and import it. Use for persisted state; use ap-connect for broken native hooks or pane identity.
---

# ap-recover — saved progress

Check `ap --version` and the relevant command's `--help`; these instructions
describe ap 1.2.4. Select the requested project's own manifest or use
`--project /explicit/ap.project.json`. Never choose another project's newest
state or infer the correct session from recency.

Start with `ap product summary` or `ap product resume` for the last completion,
blockers and next work. `ap product show` exposes full state; `ap projects
--project MANIFEST1 MANIFEST2` browses explicitly selected products.
Retain the original IDs, acceptance criteria and dependency rules. A child
roadmap item does not certify its parent. Checklist status, observed activity,
automated evidence and human confirmation remain separate claims.

## Evidence

Get a task's code fingerprint with `ap product revision ID`. After the actual
required check passes, record its real result and returned revision:

```sh
ap product evidence ID automated result.json --revision HASH
```

Do not invent a result or human confirmation. `ap` records evidence; it does
not run checks or authenticate people. Relevant code changes can stale prior
evidence and reopen verification. `ap product invalidate ID --reason 'cause'`
records an explicit invalidation when requested.

## Backup, restore and move

```sh
ap product backup --output /new/backup.json
ap product restore --backup /existing/backup.json
```

The restore command previews displacement and returns `current_hash`. When
the request authorizes restoring that snapshot, use `--expect-hash HASH` with
the just-returned hash. On concurrent change, inspect new state instead of
bypassing the guard. Displaced state is retained. Do not delete damaged files
or force overwrite symlink storage.

For a requested move, `ap product export --output /new/bundle.json` and
`ap import --input /existing/bundle.json --into /new/empty/directory` carry
product state. Import refuses an occupied destination. Raw conversations and
credentials are not included. Optional file mode selects an explicit `--file
plan.md`; it is not a prerequisite for automatic observation.
