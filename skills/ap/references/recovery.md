# Resume and recover

Use the requested project's manifest or `--project /explicit/ap.project.json`; do not
choose another project's newest state. Start with `ap product summary` or `ap product resume`
to read recent completion, blockers and next work. `ap product show` exposes full state.
`ap projects --project MANIFEST1 MANIFEST2` browses explicitly selected saved products.

## Evidence

Existing IDs, criteria and dependency rules remain authoritative. Obtain the task's code
fingerprint with `ap product revision ID`. After the actual required check passes:

```sh
ap product evidence ID automated result.json --revision HASH
```

Use the real returned HASH and retained result, not invented placeholders. A human evidence
record requires an actual relevant human confirmation. Evidence records are declarations;
`ap` does not execute checks or authenticate people. Relevant code changes can stale prior
evidence and reopen verification. `ap product invalidate ID --reason 'specific cause'`
records explicit invalidation when requested.

## Backup and restoration

```sh
ap product backup --output /new/backup.json
ap product restore --backup /existing/backup.json
```

The second command previews restoration and returns `current_hash`. If the user's request
authorizes restoring this snapshot, use `--expect-hash HASH` with the returned current hash.
Concurrent changes cause refusal; inspect the new state rather than bypassing the check.
Displaced state is retained. Do not delete damaged files or force overwrite symlink storage.

For a requested move, `ap product export --output /new/bundle.json` and
`ap import --input /existing/bundle.json --into /new/empty/directory` transport product state.
Import refuses existing destinations. Raw conversations and credentials are not included.

Optional file mode uses an explicitly selected `--file plan.md`. Consult `ap COMMAND --help`
for write operations; it is an escape hatch, not a prerequisite for automatic observation.
