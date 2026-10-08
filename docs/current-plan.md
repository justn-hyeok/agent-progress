# Current release plan

## Minimal CLI redesign — 2026-10-06

User decision (2026-10-06): the v2 work-record protocol and the 1.x native
hook/launcher observation are over-engineered. Replace them with one small CLI
the agent calls directly. Branch `feat/minimal-cli`, uncommitted.

- [x] [AP-05/minimal] Stable-numbered items, pane-keyed plans, locked atomic store, history JSONL.
- [x] [AP-06/minimal] Herdr/tmux auto-open once, reuse, `q`/`ap close` remove the pane, `ap open` reopens.
- [x] [AP-19/minimal] Published prerelease 3.0.0-alpha.1 from this branch; Homebrew formula intentionally stays on 1.2.8.
- [x] [AP-19/minimal-stable] Stable 3.0.0 on main, Homebrew formula and local installs updated.
- [ ] [AP-24/minimal] Ongoing human acceptance of the new CLI in daily use (AGENTS.md v2 block removed and the single skill installed globally on 2026-10-06).

Codex finding (live, Codex 0.160.1): commands run under the shared
app-server daemon, whose HERDR_PANE_ID belongs to the pane that started it,
and Herdr's own Codex session mapping pointed at that same wrong pane. ap now
trusts HERDR_PANE_ID only when that pane's foreground process is an ancestor;
Codex plans are keyed by CODEX_THREAD_ID and do not auto-open a pane.
Resolution without changing how Codex runs (live, Codex 0.160 daemon mode):
CODEX_THREAD_ID → thread_name from session_index.jsonl → the unique Herdr
pane running Codex titled "<thread_name> | …" inside the project. The viewer
opened directly below that Codex pane; ambiguous or unnamed threads open nothing.

Prerelease 3.0.0-alpha.1 (2026-10-06): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.0.0-alpha.1
Tag/source 29ce8d3a4458b787468e4622a5ad979f5d41f51f on feat/minimal-cli passed
Rust CI 37402981528. Archive SHA-256
0cfe8c5a8e994c311a248aadf214416ea6499e1ba508e01021c305a0d8a9621c; repack
byte-identical; package smoke passed; the downloaded asset matched the local
file. main and the Homebrew formula were not changed. Installed on this Mac in
~/.local via install.sh (rollback target 2.0.2); a fresh zsh resolves
ap 3.0.0-alpha.1. Skills are not installed anywhere yet (user decision pending).

Prerelease 3.0.0-alpha.2 (2026-10-06): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.0.0-alpha.2
Tag/source 1754a77ee7cd652c84ced0856f0f293e1a2514f2 passed Rust CI 37404075296.
Archive SHA-256 57c346a156e27663b4fdadc5175f883775b42f5a317d1d507ecd4762000dc656;
repack byte-identical; smoke passed; downloaded asset matched. Restores
list/detail/search/history/help views (live-checked in a Herdr pane). Installed
in ~/.local (rollback target 3.0.0-alpha.1). The single ap skill is installed
globally at ~/.agents/skills/ap with symlinks from ~/.claude, ~/.codex and
~/.config/opencode skills (user decision 2026-10-06).

Prerelease 3.0.0-beta.2 (2026-10-06): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.0.0-beta.2
Tag/source ec935c68ea1febcfb9900af93acb3977a5abf4a8 passed Rust CI 37405081342.
Archive SHA-256 cc525e7fd4f74f47b7d1a0925c1f6df739b36d150bc59843a2564ae085ff86ca;
repack byte-identical; smoke passed; downloaded asset matched; installed in
~/.local. Fixes the 10 code-review findings (instance + heartbeat viewers,
plan-based close, safe archives with history, done_at, single ps snapshot).
Live Herdr: close while typing a search, no reopen after q, `ap new` keeps the
viewer. 27 automated tests. Not verified: tmux, OpenCode. beta.1 (92c2672) was
the same feature set as alpha.2.

3.4.0 (2026-10-08): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.4.0 (Latest)
User decision: the progress pane opens only on `ap open` (AP_AUTO_OPEN=1 restores
first-record opening), after six panes, three of them brgr workers, had opened
viewers. Skill and Claude Code block updated; `ap skill install` replaced the
installed block in place (one marked block). Archive SHA-256
a3b05716135c57664da72fc9a9816f62936b14960b3191c17adcb68fe03b412c (repack
byte-identical, 3.3.1 → 3.4.0 smoke, downloaded asset matched); main Rust CI and
formula Homebrew install CI passed; brew and ~/.local upgraded; GJC skill copy
refreshed; agents stack verify OK. Live Herdr: records alone open nothing; `ap open`
opens and then follows.

3.3.1 (2026-10-08): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.3.1 (Latest)
Fixes plan-name collisions for non-ASCII names (`가나`/`다라` shared `__.json`):
any-script letters kept, lossy names get an FNV-1a suffix, pane keys unchanged.
Archive SHA-256 399e7f1eb6e6c0f57faca7a8e596320c623d209f7fea566d1cc5a12358db5b0e
(repack byte-identical, 3.3.0 → 3.3.1 smoke, downloaded asset matched); main
Rust CI, formula Homebrew install CI; brew upgrade/test and ~/.local on this Mac.

3.3.0 (2026-10-08): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.3.0 (Latest)
Release commit a1ab9f3 passed Rust CI on main and branch (runs 37711178330,
37711181048); archive SHA-256
a09d892ae8dddc38bcc616a67fd3c62de6c75a604831681b7d2c1cfe3578286c (repack
byte-identical, 3.2.0 → 3.3.0 upgrade/rollback smoke, downloaded asset matched).
Formula passed Homebrew install and Rust CI; brew upgrade/test and ~/.local
reinstall (a pre-review local 3.3.0 build was removed first) on this Mac.
Progress pane redesign iterated with the user: role rows, wide gradient,
completion slide, a travelling light that bends into `>` with a tail and slips
past the edge, half-block pixels (AP_HALF_BLOCKS=0 opt-out), two-decimal
percent, cool deep green theme. xhigh code review: 15 findings fixed (blocker
first, one-row status, idle-secs, load errors, sweep continuity, snap on
decrease, tick timing, real buffer tests, docs, script duplication). 57 tests.
README rewritten around a real VHS recording (docs/media, scripts/demo).
GitHub API was rate-limited during publication; CI was confirmed through the
unauthenticated public API.

3.2.0 (2026-10-08): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.2.0 (Latest)
Release commit 18b2e45 passed Rust CI 37705669392; archive SHA-256
103900b5ff6963cbea68a7d33d88b65863279496cc63ca0b924a397c6848194b (repack
byte-identical, 3.1.0 → 3.2.0 upgrade/rollback smoke, downloaded asset
matched). Formula e77e5b8 (with `ap skill install` caveat) passed Homebrew
install CI 37705835647 and Rust CI 37705835617; brew upgrade/test and
~/.local install on this Mac. Adds `ap skill install|remove`, automatic
`.agent-progress/.gitignore`, and one viewer per source pane (user-reported
bug: a repo and its worktree stacked two viewers under one pane). Live Herdr:
alternating changes from two projects switch one viewer; close, dismissal,
`ap open` and q behave per pane. A fresh Claude Code session used ap without
being told after the CLAUDE.md instruction (now the marked block written by
`ap skill install`; manual block removed, backup CLAUDE.md.bak-ap-20261008).

Remaining harnesses (2026-10-06, user-authorized login/free-model setup):
Amp (browser device login; custom-url router "Command Code (free)" on the
user's Amp account mapping all 49 Amp models to inclusionai/ling-3.0-flash-sante:free,
key piped from Keychain), Hermes (cto profile: commandcode provider with
key_cmd reading Keychain, default model ling-3.1-flash:free; telemetry left
at "No thanks"), GJC (two stale stored commandcode-goat keys disabled, not
deleted; workbuddy provider disabled; backups *.bak-ap-20261006) all opened
the viewer directly below their pane. All 15 installed harnesses are verified.

3.1.0 (2026-10-06): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.1.0 (Latest)
Release commit cdb9b7a passed Rust CI 37419786369; archive SHA-256
ccd651a9f2c887cd6d88775360a8afc8fe0cd7048841d0096cff8e7b352c0982 (repack
byte-identical, 3.0.0 → 3.1.0 upgrade/rollback smoke, downloaded asset
matched). Formula 902d483 passed Homebrew install CI 37419906765 and Rust CI
37419906745; brew upgrade + brew test and ~/.local install on this Mac.
Harness live checks in Herdr (viewer directly below the agent pane): Cursor
CLI, Copilot CLI, Cline (daemon → unique Cline pane), OMP, Gemini CLI, Devin,
pi, Antigravity CLI, Command Code, plus Claude Code, Codex, OpenCode. Skill
discovery confirmed but live use blocked by user-side state: Amp (expired
login), Hermes (first-run telemetry choice; external_dirs entry added to
~/.hermes/profiles/cto/config.yaml with backup), GJC (default model out of
credits; Composer blocks bash and its model fabricated command output). Skill
installs: ~/.gemini/antigravity-cli/skills/ap link, ~/.agents/stack/harnesses/
hermes/skills/ap link, ~/.agents/stack/harnesses/gjc/skills/ap copy;
~/.agents/stack verify.py and unit tests passed.

Stable 3.0.0 (2026-10-06): https://github.com/justn-hyeok/agent-progress/releases/tag/v3.0.0 (Latest)
Release commit 4b86bd979fe04cb1c23971f52e0d74890975e2fe fast-forwarded main and
passed Rust CI 37416614875. Archive SHA-256
be651a1087d88a0e5d5032a15b7d623ba6ff4438c9ebe3871deee2dfd22294c0; repack
byte-identical; beta.2 → 3.0.0 upgrade/rollback smoke passed; the downloaded
asset matched. Formula commit 577b1eda passed Homebrew install CI 37416793911
and Rust CI 37416793916. This Mac: `brew upgrade` to 3.0.0 and `brew test`
passed; ~/.local also runs 3.0.0 (PATH resolves ~/.local/bin/ap first).

Live verification after beta.2 (2026-10-06, installed beta.2 binary):
- tmux 3.6a (isolated server): viewer split below the source pane, updated,
  `ap close` removed the pane, no reopen while dismissed, `ap open` reopened,
  `q` closed it and stored the dismissal.
- OpenCode 2.0.20 in Herdr: HERDR_PANE_ID passed the ancestor check (bash tool
  runs under the pane's OpenCode), plan keyed to that pane, viewer opened
  directly below it. OpenCode auto-updated itself from 1.18.30 on launch.
- package_smoke.py --previous: beta.1 → beta.2 upgrade, rollback (beta.1 reads
  the beta.2 plan unchanged), re-upgrade, uninstall keeping project data and an
  unrelated bin file.

Evidence: 11 CLI integration tests and 3 render tests, fmt and Clippy pass; one live Herdr run
opened, refreshed, closed, suppressed, reopened and q-exited the viewer with
no leftover pane. No AP acceptance item is certified by this stage. The v2.0.2
source was not found on disk; the installed 2.0.2 binary was left untouched.

## Repeated pane growth hotfix — 2026-10-01

- [x] [AP-06/pane-growth] Reproduce a manually shrunk Herdr observer growing after an automatic reopen, and preserve its live height on ordinary reuse.
- [x] [AP-09/pane-growth] Verify idle and active observer reuse, explicit config resize, full Rust regression and terminal lifecycle.
- [x] [AP-19/pane-growth] Package, publish and install the verified 1.2.8 hotfix without altering the 1.2.7 release or the separate Linux candidate.

The 1.2.7 source reapplied `pane_size_percent` whenever a native hook reopened
an already-owned observer. A mocked Herdr contract reproduced a saved 40% size
growing a manually reduced 10% pane during `ap open`. An isolated real Herdr
session confirmed resize direction and ratio behavior, so the repeated
reapplication is the responsible path. Existing active and reusable-shell
observers now retain their measured height; new observers and explicit
`ap config set --pane-size` still apply the preference. The test session was
stopped after measurement. Live user panes remain untouched during the fix.

Local 1.2.8 candidate: `dist/release/agent-progress-1.2.8-macos-arm64.tar.gz`,
SHA-256 `ed52a7b7ee66e87d0cfafd3ea3bb3047291d9ad49664d8968d1953d0419ef2e0`.
The repack is byte-identical. Locked Rust tests, fmt, Clippy, release build,
setup/presentation/real-tmux PTY, and four skill validators passed. Exact
archive smoke passed authenticated Codex/Claude/OpenCode producers, temporary
upgrade, rollback and uninstall, retaining product and unrelated data. This is
local evidence; public and installed proof follows.

Published 1.2.8: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.8.
Annotated tag/source `f786a86ee7dfc4511f45af32d0ddd456ac35d5c9` passed
branch Rust CI `36810999534` and tag Rust CI `36811211943`; the downloaded
archive and checksum asset were byte-identical to the local candidate. Formula
commit `8a7ae24e9009ee3a18d87c670507e613f1998ca1` passed Homebrew install
CI `36811449347` and Rust CI `36811449351`. This Mac upgraded through
Homebrew from 1.2.7 to 1.2.8, retaining previous Cellar copies. Installed
`ap` matched the public package binary SHA-256
`02f9559dadd896eb0ecc50b76dab369ac091e0984cc6c9595711abfba69eb252`.
`brew test`, strict audit, installed setup/tmux PTY and four local Codex skill
validators passed. Installed skills matched the packaged bytes; the prior
copies were backed up at `/Users/justn/.codex/ap-skill-v128-backup.KpEdZx`.
Running user panes were left undisturbed. The separate Linux 1.3 candidate
remains unpublished, and 1.2.8 is macOS arm64 only without Apple signing or
notarization.

## Empty-session display and minimum default — 2026-10-01

- [x] [AP-06/open-on-plan] Keep the automatic observer closed for an empty native session; open it for the exact session after a goal or plan appears.
- [x] [AP-09/open-on-plan] Verify Herdr hook, Codex frontend and real tmux empty-to-plan transitions, default size, focus and opt-out.
- [x] [AP-19/open-on-plan] Record final local evidence and keep the pending Linux candidate separate.
- [x] [AP-19/open-on-plan-release] Verify the exact 1.2.7 source/archive, publish the tag and GitHub Release, then upgrade and verify this Mac through Homebrew.

New project-local presentation settings default to the measured Herdr minimum of 10%.
Explicit saved sizes continue to win. Native goal/plan state, not a project roadmap or
session registration alone, triggers automatic opening. Manual `ap open` still works.
The Linux 1.3 candidate remains in its separate worktree; this patch is based on
published 1.2.6 and does not include Linux support.

Local verification: `HERDR_ENV=0 cargo test --all-targets`, fmt, Clippy with
warnings denied, `git diff --check`, interactive setup PTY and real tmux/PTY
all passed. The Herdr hook contract is isolated with a CLI mock: SessionStart
does not split, a later plan does, and `AP_AUTO_OPEN=0` still prevents opening.
The real tmux fixture keeps one pane until a Claude plan arrives, then verifies
the default 10% size, focus, live resize and cleanup. Its Codex WebSocket
fixture starts empty, writes a native goal to the exact session's SQLite DB
without another RPC event, and verifies automatic opening and goal visibility
in the selected snapshot. Other-session goals do not trigger it. The skill
source validator passed. These scenario checks are local/fixture results,
separate from the authenticated package smoke and public release evidence below;
human acceptance was not repeated. Reconcile the separate
Linux candidate with 1.2.7 when its release work resumes; neither worktree
has been modified by the other task.

1.2.7 macOS arm64 candidate archive:
`dist/release/agent-progress-1.2.7-macos-arm64.tar.gz`, SHA-256
`a1062287d6764fd162e0af947e1f85b9a031f425aeb2acbd3e29a7daae890cd4`.
The independent repack was byte-identical. Full locked Rust tests, fmt,
Clippy, release build, setup/presentation/real-tmux PTY and all four source
skill validators passed. The extracted archive passed isolated installation,
native Codex/Claude/OpenCode producer checks, update, rollback and uninstall
with product and unrelated files preserved. The package smoke used this Mac's
authenticated agents in a temporary prefix. Public release and installed
evidence follows below.

Published 1.2.7: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.7.
Annotated tag/source `065217222bf04a4a642ae6993177497b7bd8359f` passed
branch CI `36808740342` and tag CI `36808959686`. The downloaded archive and
checksum asset were byte-identical to the verified local files. Formula commit
`9fa455f4737b8753bf67d24d9cc1951a8e8806bb` passed Homebrew install CI
`36809257673` and Rust CI `36809257694`. Local Homebrew upgraded 1.2.6 to
1.2.7 while retaining older Cellar copies; installed `ap` matched the public
binary SHA-256 `21a1a73af8f4d569c19de50a244182b33a4f55614294c87668ecfeca413c3e98`.
`brew test`, strict audit, and installed setup/tmux PTY passed. The four
installed Codex skills matched the packaged bytes and passed validation;
prior copies were backed up at
`/Users/justn/.codex/ap-skill-v127-backup.ffOvHW`. Existing user panes and
project data were not modified. Linux 1.3 remains an independent unpublished
candidate; 1.2.7 contains macOS arm64 only and is unsigned/unnotarized.

## First-run pane sizing — 2026-09-30

- [x] [AP-06/pane-size] Add project-local first-use size selection and `ap config set --pane-size` with backward-compatible defaults.
- [x] [AP-09/pane-size] Apply size immediately to the exact owned Herdr/tmux observer and verify minimum, position, focus and isolation.
- [x] [AP-19/pane-size-candidate] Prepare and locally verify the 1.2.6 archive, Homebrew formula, skills and first-run documentation.
- [x] [AP-19/pane-size-release] Publish and install 1.2.6 after release authorization; keep 1.2.5 immutable.

The first setup runs in the project root with `ap setup` (also `ap config setup`),
offering the measured Herdr minimum 10%, 20%, 30% and 40%, plus direct 10–50%
input. `ap config set --pane-size N` is the noninteractive later-change path.
Old JSON/YAML settings retain 30%; existing color, position and auto-open
choices survive setup. Homebrew itself stays noninteractive and will show the
post-install command in the 1.2.6 formula caveat. A saved change made in the
exact source pane resizes only its owned progress sibling now; another terminal
saves it for the next opening.

The live installed Herdr test requested a 98/2 split in an isolated owned
worktree pane. Herdr clamped it to 90/10 (59/7 rows in a 66-row area) without
changing focus; the temporary test pane was closed. The CLI range follows
that observed current-version minimum. Mocked exact-session Herdr tests cover
new below/above splits, immediate 40% and 10% updates, protected layout and
focus. Real tmux/PTY tests cover initial and live sizes both below and above.

Verified candidate: `dist/agent-progress-1.2.6-macos-arm64.tar.gz`,
SHA-256 `f52e82a53f8a77766de23039637434596eb818f9221926322a398f31dd905aea`.
The repack matched byte-for-byte. Full Rust tests, fmt, Clippy, four source
skill validations, release-binary first-run/presentation/tmux PTY scenarios,
the extracted archive's first-run UI, and exact archive smoke passed. The
archive smoke used disposable authenticated Codex/Claude/OpenCode producers,
then verified update, rollback and uninstall with product/unrelated data
preserved. This is local/fixture evidence, distinct from public distribution
and human acceptance. Independent `codex review` could not complete because
the local review CLI recursively invoked another review with an unsupported
default model; the targeted manual diff review and required checks passed.

Published release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.6.
Annotated tag/source 2cd7e52fa45c95b607a4c4f778d8e26d4dde13e6 passed
Rust CI (source 36672311615; tag 36672577686). The public archive and
checksum asset matched the verified local SHA-256. Formula commit
cd60db6894f8094784ce2acf2abf437701f5ea48 passed Homebrew install CI
36672712554 and Rust CI 36672712557. This Mac upgraded to Homebrew 1.2.6;
installed binary bytes and four `share/agent-progress/skills` folders matched
the tested source, and `brew test`, strict audit, installed doctor and installed
first-run PTY scenario passed. The prior 1.2.5 Cellar copy was retained.
Local Codex skills were backed up at
`/Users/justn/.codex/ap-skill-v126-backup.i5UxwO`, updated to source and
validated. Running user panes and native agent settings were not changed.

## OpenCode live QA reliability — 2026-09-30

- [x] [AP-09/opencode-qa] Require an actual successful native `todowrite` event at every OpenCode step and remove the Claude-specific file-tool wording.
- [x] [AP-09/opencode-package] Make the package-smoke OpenCode model selectable and verify a fresh installed 1.2.5 run end to end.
- [x] [AP-19/opencode-handoff] Record the live result and its limits without changing the published 1.2.5 binary.

The previous `glm-5.3-flash` package run timed out; a `gpt-6-luna` run with
the old QA wording answered one progress request without a tool call.
The revised fixture asks OpenCode for native todo updates only, uses an
available file-editing tool instead of Claude's `Write` name, and fails
explicitly if `todowrite` did not complete. Two fresh installed-ap 1.2.5
checks passed without corrective prompts: direct OpenCode 1.18.30 native
todos/file work/resume (0/2 → 1/2 → 2/2 with stable IDs), and the full archive
package smoke including Codex, Claude, OpenCode, temporary upgrade, rollback
and uninstall. The package-smoke model can be changed per run with
`--opencode-model`; it does not edit user configuration. This removes the
observed QA prompt/model selection failures; it does not guarantee a provider
will never time out or that every model will always choose a tool.

## Skill bundle release — 2026-09-30

- [x] [AP-19/skill-package] Include `$ap` and the three focused skills in the 1.2.5 archive and Homebrew formula.
- [x] [AP-09/skill-release-check] Verify the source, package checksum, exact skill bytes, local update/rollback and terminal fixtures.
- [x] [AP-19/skill-release] Publish the matching 1.2.5 tag, assets and Homebrew formula, then verify the public install.

Candidate archive: `agent-progress-1.2.5-macos-arm64.tar.gz`, SHA-256
`01af4d0d6327aeda74f41aed53e0713c27d80fd98d05a3451abcd8ac0cc2ed68`.
The reproducible repack was byte-identical. Rust tests, fmt, Clippy, skill
validation, release-binary PTY presentation and terminal fixtures passed with
the same `HERDR_ENV=0` isolation used by CI. The first local test attempt
inherited the active Herdr environment and its bridge fixtures failed; the
documented CI environment passed. The exact archive contained all four skills
and passed member checksums. An isolated install upgraded 1.2.4 to 1.2.5,
rolled back to 1.2.4, and uninstalled cleanly. Full package smoke reached
real Codex and Claude native producers, then OpenCode's first
`opencode-go/glm-5.3-flash` model request timed out at 120 seconds. A later
bounded OpenCode retest is recorded below.
The released binary logic is unchanged from 1.2.4.
Published release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.5.
The annotated tag resolves to source commit b9e8064d845164662698012464d923cbecfb6415;
its Rust CI passed (36612926812), as did the source push CI (36612630502).
The public archive and checksum asset matched the local candidate SHA-256.
Formula commit a303efa675b4ec1a1879aac68a843c80c57c7516 passed Homebrew
install CI (36613053687) and Rust CI (36613051967). This Mac upgraded via
Homebrew to 1.2.5, and the
installed binary and four `share/agent-progress/skills` folders matched the
verified source bytes. `brew test`, `brew audit --strict`, installed doctor
`--shell zsh --strict` and local Codex skill validation passed. Local Codex
skills were backed up and updated to the matching 1.2.5 source. Homebrew's
automatic cleanup removed the older local Cellar copies; project data and
running panes were not changed.

Follow-up OpenCode 1.18.30 live check used the installed ap 1.2.5 and
`opencode-go/gpt-6-luna` in a private disposable project. Native `todowrite`
was observed at 0/2 → 1/2 → 2/2, with stable task IDs, the same exact session
resumed in a new process, one bridge stream, and a native test-file edit.
`ap compatibility --agent opencode` also passed its CLI interface check.
The model skipped `todowrite` on the first progress prompt (state stayed 0/2);
one explicit corrective prompt in that same session produced 1/2, and the
next process completed 2/2. This proves the native connection and state path,
not reliable tool selection for every prompt or the timed-out flash model.
Raw session and provider records remain private and outside Git.

## Focused ap skills — 2026-09-30

- [x] [AP-06/skill-split] Split connection, theme and recovery into separately invokable skills while retaining the existing `$ap` entrypoint.
- [x] [AP-09/skill-split] Validate source and local installed copies against ap 1.2.4 help and the skill validator.
- [x] [AP-19/skill-split] Update README installation and invocation guidance; record local installation state.

`skills/ap-connect`, `skills/ap-theme` and `skills/ap-recover` are standalone
Codex skills. The existing `skills/ap` keeps its detailed references so earlier
single-skill installations continue to work. The four source folders passed
skill-creator validation; the locally installed copies matched the source
byte-for-byte and passed the same validation. The installed ap 1.2.4 help was
checked for the documented connection, doctor, shell, config and product
commands. The existing local `$ap` matched the main source before replacement;
its prior copy is retained at `/Users/justn/.codex/ap-skill-backup.Xtn6Et`.
The three new names appeared in the next Codex turn's available-skill catalog.
The later 1.2.5 package/release is recorded above. Task-level behavior of
each new skill has not been exercised separately.

## Connection lock stability — 2026-09-30

- [x] [AP-09/connection-lock] Retry brief project-connection lock contention without weakening receipt/content checks.
- [x] [AP-19/connection-hotfix-release] Verify and publish a separate 1.2.4 patch release, leaving 1.2.3 tag/assets immutable.

Latest 1.2.3 documentation-only CI repeated a transient connection-lock busy failure
seen in earlier release CI. The immutable source/tag and Homebrew CI passed; this
follow-up improves the actual CLI setup/remove path rather than only rerunning CI.
Local gate: 132 Rust tests, fmt/Clippy, release build, tmux/PTY and presentation
scenarios, source skill validation, exact archive install/uninstall and byte-identical
repackaging passed. Codex/Claude/OpenCode lock fixtures each waited for and recovered
from a short lock; existing receipt, content and symlink checks remained. SHA-256:
1261c3a31a1781bb40e70d6d50c41f4486d2d5861719134d59b4c5ae95e23106.
Published release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.4.
Tag/source 8173d7e86ac9d90afe1cf29432ae5c81c177f4f5 passed Rust CI
(source run 36607607178; tag run 36608077826). Homebrew formula
6484740d754993177841c0e2862e5398a14b4e35 passed install CI 36608169327
and Rust CI 36608169336. Downloaded archive matched the SHA-256 above.
This Mac upgraded via brew to 1.2.4; brew test/audit and installed doctor --strict
passed. Installed binary matched the tested release binary. The global ap skill was
backed up/validated against its prior-source baseline, and managed zsh setup stayed
current without rc changes. Prior managed binary versions and live user panes were
retained. The 1.2.3 release note now points to this connection-lock fix.

## Nested passive-project isolation — 2026-09-30

- [x] [AP-06/passive-boundary] Keep a manifest-free child project separate from its ancestor product plan, including when its own connection is removed.
- [x] [AP-09/passive-native-check] Verify isolated project projection and live installed Claude/OpenCode native plan/resume paths.
- [x] [AP-19/passive-hotfix-release] Publish and install a checked patch release without changing the existing 1.2.2 tag/assets.

Readiness check reproduced a 1.2.2 fault in an isolated nested fixture: follow attached
the child plan to its ancestor's product and wrote ancestor progress. No user data was
changed by that reproduction. The fix records an explicit passive root, respects owned
connection and nested Git boundaries, and continues to select a declaration at the
project's own root first. Prior product state is preserved; it is not silently rewritten.
Target a new 1.2.3 release after checks, keeping v1.2.2 immutable.
Published hotfix: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.3.
Tag/source e0a0e5f19154d2677015ee418c5cf446f6e63a5f passed Rust CI
(branch run 36605833772; tag run 36606160641). Homebrew formula
f826286b14f3e63026a81d7793d73f42bbf3382f passed install CI 36606223342
and Rust CI 36606223423. Downloaded archive matched SHA-256
da1dabcfa3ce7509e4dfd1597739dad63556f179f44b9089beea246cd3d320e6.
Local brew upgrade/test/audit and exact installed binary match passed; the ap skill was
backed up/upgraded and validated. Existing managed zsh setup stayed current, prior
managed binary version was retained, and running user panes/credentials were untouched.
The v1.2.2 release note discloses its nested-project issue and links this hotfix.
Local candidate verified: 131 Rust tests, fmt/Clippy, release build, PTY, skill validation,
exact archive installation/removal and byte-identical repackaging. Archive SHA-256:
da1dabcfa3ce7509e4dfd1597739dad63556f179f44b9089beea246cd3d320e6.
Real Claude Code 2.1.284 (haiku native tasks) and OpenCode 1.18.30
(opencode-go/glm-5.3-flash native todos) each completed 0/2 → 1/2 → 2/2 with
stable IDs and new-process resume in manifest-free private fixtures. Their source
development build was tested before the 1.2.3 version bump. A Codex review found
concurrent first-hook marker publication could drop an event; the idempotent verified
marker fix and parallel regression passed. Previously cached product state is not
automatically rewritten. The 1.2.2 GitHub release notes now disclose that issue.

## Diagnostic completion and release — 2026-09-30

- [x] [AP-09/doctor-complete] Extend read-only doctor checks for connections, shell setup and exact terminal slots; preserve default exit behavior and add opt-in strict checking.
- [x] [AP-06/shell-upgrade-status] Report current/outdated/absent/partial/conflict shell setup and a safe next action without altering user configuration.
- [x] [AP-19/refinement-release] Verify, publish and install the complete refinement package with matching GitHub/Homebrew assets.

The user requested both previously deferred diagnostics and publication. Continue in
the existing skill/CLI task worktree, preserve its prior changes, and include the earlier
manifest-free/opt-out/preview improvements. Target the next patch release, 1.2.2.
Local candidate verified: 129 Rust tests, fmt/Clippy, release build, release-binary
tmux/PTY, skill validation and exact archive install/uninstall. Repacking was byte-identical.
SHA-256: a26b942618b62ea4ff3da97b64155d2a7649abdf50c58afc5a826b66505b1e43.
Independent Astra review found two diagnostic misses (hidden non-UTF8 managed bash file,
invalid roadmap without cached state); both were reproduced, fixed and covered by tests.
Published release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.2.
Tag/source commit 0822dd5324e563f0c1f2fefba9e6c39bac175a7b passed Rust CI
(run 36599817895 and tag run 36600215040). Homebrew formula commit
691988cec8475c91377fbb729934387cbd6651e2 passed install CI (36600288216).
Its Rust CI run 36600288191 passed on full rerun: the first attempt hit transient
connection-lock busy (os error 35) in a concurrent test; neither tag nor source changed.
Published download matched the local archive hash above. Homebrew upgraded this Mac to
1.2.2; brew test/audit and installed doctor --agent codex --shell zsh --strict passed.
Installed binary matched the tested release binary. Managed zsh setup remained current,
global ap skill updated from the exact prior source with private backup and validation,
and live user panes/native credentials were left intact. Native producer tests for the
new manifest-free path and human acceptance were not separately performed.

## Skill and CLI refinement — 2026-09-29

- [x] [AP-06/skill-cli-audit] Review skill/CLI contract gaps with user-requested Astra.
- [x] [AP-09/skill-cli-contract] Verify the findings and implement a small evidence-backed first improvement set.
- [x] [AP-19/skill-cli-docs] Align skill, command help and tests with the verified implementation.

Base: bd5a875148eb746a42222821731ac597925fe9da (published 1.2.1 follow-up).
Workspace: /Users/justn/dev/agent-progress-skill-cli. Astra uses the native Codex subagent
surface, gpt-6-astra at medium effort, for the initial review and a bounded connection
state helper implementation. Root owns CLI/bridge integration, reproduction, tests and
skill alignment. Global setup changes and a new publication are not delegated. Product choices that
cannot be resolved from source evidence remain explicit; routine verified fixes can proceed.

Reproduced on installed 1.2.1 in isolated fixtures: all three preview commands fail without
a manifest, and all three launchers overwrite AP_AUTO_OPEN=0 with 1. The first implementation
set makes product mapping optional, retains explicit opt-out, and adds read-only connection
ownership state to preview. Existing public JSON fields and protected apply/remove remain.
Final local verification: 118 Rust tests, fmt/Clippy, release build, real tmux/PTY fixture
and source skill validation passed. Astra's independent review found a broken-manifest
auto-discovery fallback; it was reproduced and fixed with a regression. Explicit root
matching prevents declaration links from writing another product's state. Native settings
and global installed 1.2.1/skill were not changed. Work is local/unpublished; doctor and
shell-status expansion are the next candidates, not part of this completed first set.

## 1.2.1 release — 2026-09-29

- [x] [AP-19/portable-package] Version and verify the exact 1.2.1 archive and installation.
- [x] [AP-19/portable-publish] Publish the matching source commit, tag, assets and Homebrew formula.
- [x] [AP-19/portable-installed] Verify published checksums, CI and the installed 1.2.1 binary.

Exact 1.2.1 archive installation/removal and its public shell/compatibility/real-tmux
fixture paths passed. Repacking was byte-identical. SHA-256:
d4fedf795f36f6fd78c3e367dcc00caf2c3a1df51832b16becfeb2160a4588ef.
Release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.1.
Tag/source: 8e0b86ee1c9c4baeecec7e25eb7e5c965ddde99c (Rust CI passed).
Homebrew formula: 6005867344044e16d3f5bbfdc9e046900ff69743 (install CI passed).
Published download matched the local archive hash. Homebrew upgraded this Mac to 1.2.1;
brew test/audit and installed CLI/RPC checks passed. The installed binary matched the
release binary. Existing managed zsh setup was backed up/upgraded, and installed ap
skill updates passed validation after checking against the prior source baseline.
Active shells need a new shell or explicit rc reload; running user panes were retained.

## Portable connections — 2026-09-29

- [x] [AP-06/shell-portable] Add bash/fish preview/install/status/remove without replacing user definitions or executables.
- [x] [AP-06/terminal-portable] Automatically show the exact native session outside Herdr using optional tmux.
- [x] [AP-09/compatibility] Detect version changes and check native connection/plan contracts, separating live and fixture evidence.
- [x] [AP-19/portable-verification] Verify real shells/terminal lifecycle and update usage documentation.

Scope: repository changes and isolated verification. Existing global zsh configuration
and user panes are preserved. No additional OS packaging or Apple signing is included.
Implementation is released as 1.2.1. Real zsh/bash/fish, tmux/PTY fixtures, native
markdown producers and review fixes are recorded in verification/portable.md.
Final local verification: 104 Rust tests, fmt/Clippy, release build, release-binary
tmux/PTY and presentation scenarios passed. Native CLI/RPC checks passed on all three
installed agents. Source skill metadata passed validation. Review findings on native
exit status, slot cleanup, Bash login precedence and unobservable Codex flags were fixed
and covered by regression scenarios. The release checklist above tracks public/installed state.

## Shell integration — 2026-09-29

- [x] [AP-06/shell] Implement optional zsh install/status/remove with preservation and backup.
- [x] [AP-09/shell] Verify routing, quoting, native escape, non-interactive passthrough and safe removal in real zsh.
- [x] [AP-19/shell] Release/install the verified shell integration package.

The user's rc was backed up and updated after isolated tests. No original Codex executable
was replaced. Existing sessions need a new shell or explicit rc reload. Native project
trust is not bypassed. Tests cover edited blocks, symlinks, exact original bytes and
post-install user edits; the public command works without a plan file.
Actual shell spelling `codex resume` opened the correct native Codex 0.157.1 session
through server 0.158.0 and automatically opened the observer; focus and 28/12 heights
were retained. 93 tests passed. Non-interactive command bypass was added after review.
Tag commit 552283697849bee045c01367134b7ee7b0d00916 passed CI.
Release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.2.0
SHA-256: 657bc6f1b941fd14d61ffb584fef6f0b6e01e0ab2cff6efa2842690289cb449d.

## Native connection follow-up — 2026-09-29

- [x] [AP-09/native-recheck] Recheck installed Codex, Claude Code and OpenCode with actual native inputs.
- [x] [AP-06/daemon] Verify the launcher connection fix with shared-server clients, new threads and resume.
- [x] [AP-19/connection-release] Verify the final connection fix and update support documentation and release assets.

Claude Code 2.1.274 and OpenCode 1.18.30 passed native task/todo updates, restricted fixture
file work and new-process resume against ap 1.1.0. Codex 0.157.1 passed native Plan/report,
goal lifecycle, cross-session IDs and app-server restart persistence. This is fresh native
adapter evidence, not proof that every installation or future harness version is supported.

Shared-server frontends reproduce missing pane/session identity in an isolated Herdr
fixture. Hook inputs have session/transcript data but no client PID or pane identity;
the inherited pane environment is insufficient. Herdr client-side identity delivery is
implemented in agent-progress itself; Herdr source changes were not needed for the
launcher route. Plain shared `codex` startup outside that route is not claimed fixed.
Final native lifecycle checks passed: two distinct client sessions in one project;
cross-directory resume; new thread reusing the observer; the other source unchanged;
focus and 28/12 pane heights retained. 89 Rust tests, fmt/Clippy, PTY checks and exact
archive install/removal passed; update/rollback also passed on the prior same-version
candidate. Final tag commit 78130cd9a706b05f135d93635af5b4de72640672 passed GitHub CI.
Release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.1.1
Archive SHA-256: 18fad7f074fa1bb615dd78028401c658572aed9ca99d8ff56fba0c316580796d.
Apple signing/notarization remain excluded.

Agent skill follow-up completed: skills/ap provides the $ap entrypoint and focused
connection, theme and recovery references. Installed locally in the Codex skills
directory; both source and installed copies passed skill-creator validation.
Reference links and standalone config/file commands passed against ap 1.1.0 in an
isolated temporary project. No active user panes or native agent settings were changed.

Homebrew follow-up completed: Formula/agent-progress.rb is published as a custom tap.
Real GitHub download/install/reinstall and brew test passed on macOS arm64; brew style
and audit passed. The Homebrew install CI passed for bd3afcdf321703e17ebe92d929b02074a3593865.
Upgrade/uninstall commands are documented. Existing v1.1.0 assets and checksum remain unchanged.

Goal: publish the 1.1.0 source and unsigned macOS arm64 package to GitHub.

- [x] [AP-16/theme-1.1.0] Implement YAML, presets, brightness and state colors.
- [x] [AP-19/privacy-1.1.0] Exclude credentials, session records and local captures.
- [x] [AP-19/verify-1.1.0] Verify source tests, TUI and exact installed package.
- [x] [AP-19/publish-1.1.0] Publish the matching commit, tag and release assets.

Release: https://github.com/justn-hyeok/agent-progress/releases/tag/v1.1.0
Tag commit: 1db02e38e8cf7af8d65cc301b0121c879499b8ef.
83 Rust tests, fmt, Clippy, PTY checks and exact archive installation passed locally.
GitHub CI passed for the tag commit. Repacking produced identical archive bytes.
Archive SHA-256: 171fbef452771012609a8cf54a215bccedc7a25e3149ba21e1f89fade8c3cac2.

Original acceptance criteria remain in product-completion-plan.md. A release substep
does not independently certify the full parent AP item.
