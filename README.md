# agent-progress

**에이전트가 지금 무엇을 하고 있고, 다음에 무엇을 할지 한눈에 보는 터미널 진행 창.**

에이전트가 `ap` 명령으로 계획을 기록하면, 에이전트 pane 아래에 진행 창이 자동으로 열려
목표·체크율·현재 작업·막힌 이유를 보여줍니다. 사용자는 아무것도 등록하지 않습니다.

## 사용

```sh
ap goal "로그인 버그 수정"
ap add "원인 재현" "토큰 갱신 수정" "회귀 테스트"   # 1, 2, 3
ap start 1
ap done 1
ap block 2 "스테이징 키 필요" --needs user
ap cancel 3 "범위 밖"
ap status            # 현재 계획
ap history           # 변경 이력 (JSONL)
ap new               # 현재 계획 보관 후 새로 시작
ap open / ap close   # 진행 창 다시 열기 / 닫기
ap view              # 현재 터미널에서 진행 창 실행
```

- 항목은 출력에 보이는 번호나 정확한 제목으로 고릅니다. 번호는 한 번 정해지면 바뀌지 않습니다.
- 체크율은 완료/(전체 − 취소)이며 제품 완성도나 남은 시간이 아닙니다. 항목이 없으면 `미정`입니다.
- 완료는 에이전트의 보고입니다. 검증 여부는 별도로 확인해야 합니다.

## 어떤 계획인지 찾는 방법

추측하지 않고 환경변수를 씁니다.

| 우선순위 | 키 | 진행 창 |
| --- | --- | --- |
| `--plan NAME` / `AP_PLAN` | 지정한 이름 | 호출한 pane 아래 |
| `HERDR_PANE_ID` (조상 프로세스 확인) | `herdr-<pane>` | Herdr pane 아래 자동 |
| `TMUX_PANE` | `tmux-<pane>` | tmux pane 아래 자동 |
| `CODEX_THREAD_ID` | `codex-<thread>` | 스레드 이름이 제목인 Codex pane 아래 자동 |
| 그 외 | `default` | `ap view`로 직접 |

계획은 Git 루트(없으면 현재 디렉터리)의 `.agent-progress/plans/<key>.json`에 저장됩니다.
`.agent-progress/.gitignore`를 자동으로 만들어 어느 프로젝트에서도 커밋되지 않습니다.
Git 루트 기준이므로
하위 디렉터리에서 실행해도 같은 계획을 씁니다. 같은 pane ID가 다른 terminal에 재사용되면
이전 계획은 보관되고 새 계획으로 시작합니다.

`HERDR_PANE_ID`는 상속된 값일 뿐이므로, 그 pane에서 실행 중인 프로세스가 실제로 `ap`의
조상 프로세스일 때만 믿습니다. Codex는 명령을 공유 데몬에서 실행해서 이 값이 다른 pane을 가리킵니다.
그래서 Codex는 다음처럼 찾습니다(Codex 실행 방식은 바꾸지 않습니다).

1. 명령 환경의 `CODEX_THREAD_ID`로 `~/.codex/session_index.jsonl`에서 스레드 이름을 찾습니다.
2. Herdr에서 Codex가 실행 중이고 제목이 `<스레드 이름> | …`이며 작업 폴더가 프로젝트 안인 pane을 찾습니다.
3. 정확히 하나일 때만 그 pane 아래에 진행 창을 엽니다. 없거나 여러 개면 열지 않습니다.

스레드 이름은 첫 응답 뒤에 정해지므로, 그 전의 기록은 저장만 되고 다음 기록 때 창이 열립니다.
계획은 `codex-<thread>`로 세션별로 저장됩니다.

## 지원 하네스

`ap`는 셸 명령을 실행할 수 있는 에이전트라면 어디서든 기록할 수 있습니다. 아래는 Herdr에서 진행 창이
에이전트 pane 바로 아래에 자동으로 열리는 것을 실제로 확인한 목록입니다(2026-10-06).

| 하네스 | pane 확인 방식 | 스킬 위치 |
| --- | --- | --- |
| Claude Code | 조상 프로세스 | `~/.claude/skills` |
| Codex 0.160 | 공유 데몬 → 스레드 이름 제목 | `~/.codex/skills` |
| OpenCode 2.0 | 조상 프로세스 | `~/.config/opencode/skills` |
| Cursor CLI | 조상 프로세스 | `~/.agents/skills` |
| Copilot CLI | 조상 프로세스 | `~/.agents/skills` |
| Cline 3.0 | 공유 데몬 → 프로젝트의 유일한 Cline pane | `~/.agents/skills` |
| OMP | 조상 프로세스 | `~/.agents/skills` |
| Gemini CLI 0.62 | 조상 프로세스 | `~/.agents/skills` |
| Devin CLI | 조상 프로세스 | `~/.agents/skills` |
| pi | 조상 프로세스 | `~/.agents/skills` |
| Antigravity CLI (agy) | 조상 프로세스 | `~/.gemini/antigravity-cli/skills` |
| Command Code | 조상 프로세스 | `~/.agents/skills` |
| Amp | 조상 프로세스 | `~/.agents/skills` |
| Hermes | 조상 프로세스 | `skills.external_dirs` |
| GJC | 조상 프로세스 | 스캔 폴더에 실제 디렉터리 |

GJC는 스캔 폴더 밖을 가리키는 링크를 거부하므로 스킬을 실제 디렉터리로 복사해 둡니다. Hermes는 큐레이터가
관리하는 폴더 대신 읽기 전용 `skills.external_dirs`에 등록합니다. tmux는 Herdr 대신 쓸 수 있는 터미널로
확인했습니다.

Cline은 세션 식별 정보 없이 공유 데몬에서 명령을 실행하므로, 같은 프로젝트에서 Cline pane을 두 개 이상
열면 진행 창을 자동으로 열지 않습니다.

## 진행 창

- 첫 변경 때 열리고 이후에는 재사용합니다. 파일을 0.5초 간격으로 확인해 갱신합니다.
- 줄은 역할로 나뉩니다: `지금 ›` 진행 중, `막힘 ›`, `다음 ›`, `완료 ›` 마지막 완료, 맨 아래 흐린 `파일 ›`.
  진행 중 항목이 동시에 둘 이상이면 계획과 실제 작업이 어긋난 것이므로 각각 경고색으로 보여줍니다.
- 배경 채움은 진행 경계에서 넓게 퍼지는 그라데이션으로 트랙에 녹아들고, 100%는 꽉 채웁니다. 단계를 끝내면
  채움이 새 위치까지 짧게 밀려갑니다. 마지막 기록 후 `AP_IDLE_SECS`(기본 300초) 동안만 은은한 빛 띠가
  채움 안을 왼쪽에서 경계 쪽으로 흘러 진행 방향을 보여주고, 그 뒤에는 멈춥니다. 빛은 "최근에 기록됨"을
  뜻할 뿐 에이전트가 실행 중이라는 증거는 아닙니다. 움직일 때만 초당 8프레임으로 그립니다.
- pane 하나에는 진행 창이 하나만 뜹니다. 같은 pane에서 다른 프로젝트(예: 레포와 worktree)의 계획을
  기록하면 새 창을 열지 않고 마지막으로 바뀐 계획으로 화면을 바꿉니다. pane별 상태는
  `~/.local/state/agent-progress/panes/`에 있습니다.
- 상태를 보는 데 키는 필요 없습니다. 필요할 때 `↑↓`/`j k` 목록, `Enter` 상세, `/` 검색,
  `f` 미완료만, `h` 변경 이력, `?` 도움말, `Esc` 진행 막대로 돌아가기를 씁니다.
- 진행 창에서 `q`를 누르거나 `ap close`를 실행하면 창이 닫히고 자동으로 다시 열리지 않습니다.
  Herdr에서 pane을 직접 닫아도 같습니다. `ap open`으로 다시 엽니다.
- 높이는 `AP_PANE_SIZE`(10–50, 기본 10%)로 정합니다. 자동 열기는 `AP_AUTO_OPEN=0` 또는 `--no-view`로 끕니다.
- 창 열기에 실패해도 기록은 저장됩니다.
- 파일이 손상되면 덮어쓰지 않고 오류를 보고하며, 진행 창은 마지막 정상 상태를 표시합니다.

## 에이전트가 스스로 쓰게 하기

```sh
ap skill install --dry-run   # 바뀔 내용만 보기
ap skill install             # 설치
ap skill remove              # ap가 설치한 것만 되돌리기
```

- 스킬([skills/ap/SKILL.md](skills/ap/SKILL.md))을 `~/.agents/skills/ap`에 설치합니다. Cursor·Copilot·Cline·
  OMP·Gemini·Devin·pi·Command Code·Amp는 이 폴더를 직접 읽습니다.
- `~/.agents/skills`를 읽지 않는 Claude Code·Codex·OpenCode·Antigravity의 스킬 폴더가 있으면 링크를 겁니다.
- Claude Code는 목록에 있는 스킬을 스스로 잘 꺼내 쓰지 않으므로 `~/.claude/CLAUDE.md`에 표시된 지침 블록을
  추가합니다. 기존 파일은 `CLAUDE.md.ap-backup-<시각>`으로 백업합니다.
- ap가 설치하지 않은 같은 이름의 스킬이나 링크는 건드리지 않습니다. GJC와 Hermes는 별도 등록이 필요합니다.

## 개발

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

3.0.0 이전의 훅·런처·v2 작업 기록 방식은 Git 이력과 [CHANGELOG](CHANGELOG.md)에 남아 있습니다.
