# 운영과 복구 (3.x)

1.x의 훅·런처·셸 연결과 2.x의 작업 기록 프로토콜에 대한 운영 문서는 Git 이력
(`git show v1.2.8:docs/operations.md`)에 남아 있습니다. 이 문서는 3.x 기준입니다.

## 구성 요소

- **CLI** `ap`: 에이전트가 계획을 기록합니다. 매 명령은 잠금 아래 읽고-바꾸고-원자적으로 저장하며,
  변경마다 이력에 한 줄을 남깁니다.
- **진행 창** `ap view`: 계획 파일을 0.5초 간격으로 확인해 그립니다. 자동으로 연 진행 창은
  `--pane-state`를 따라가며, 그 pane에서 마지막으로 바뀐 계획을 보여줍니다.
- **스킬** `skills/ap`: 바이너리에 내장되어 `ap skill install`이 설치합니다.

## 상태 파일

| 경로 | 쓰는 쪽 | 비고 |
| --- | --- | --- |
| `<git 루트>/.agent-progress/plans/<key>.json` | CLI | 원자적 교체, `<key>.lock`으로 직렬화 |
| `<key>.history.jsonl` | CLI | 변경마다 `{"at","event","progress"}` 한 줄 |
| `<key>.<시각>-<n>.archived.json` (+ `.archived.history.jsonl`) | `ap new`, terminal 재사용 | 덮어쓰지 않음 |
| `<git 루트>/.agent-progress/.gitignore` | CLI | `*` 한 줄, 처음 한 번만 생성 |
| `~/.local/state/agent-progress/panes/<host>-<pane>.json` | CLI, 진행 창 | 보여줄 계획, 진행 창 ID, 닫음 여부, terminal ID |
| `…/panes/<host>-<pane>.viewer.json` | 진행 창 | 2초마다 갱신하는 생존 신호 (6초 지나면 꺼진 것으로 봄) |

## 진행 창 수명

1. 기록이 바뀌면 CLI가 pane 상태의 `plan`을 그 계획으로 바꿉니다. 살아 있는 진행 창은 그 계획으로 화면을
   바꿉니다.
2. 진행 창이 없고 닫음 표시도 없으면, pane 상태에 새 진행 창 ID를 먼저 기록한 뒤 진행 창을 띄웁니다.
   아래에 셸만 실행 중인 pane이 있으면 그 pane에서, 없으면 새로 나눠서 실행합니다.
3. `ap close`는 pane 상태에서 진행 창 ID를 지우고 닫음 표시를 남깁니다. 진행 창은 자기 ID가 빠진 것을 보고
   스스로 종료합니다. 키 입력이나 `kill-pane`을 보내지 않습니다.
4. 진행 창에서 `q`를 누르면 같은 닫음 표시를 남깁니다. Herdr에서 pane을 직접 닫아 생존 신호가 끊기면
   그것도 닫은 것으로 봅니다. `ap open`만 다시 엽니다.
5. 같은 pane ID가 다른 terminal에 재사용되면 pane 상태와 계획을 새로 시작합니다.

## 진단

```sh
ap status --json                                        # 지금 계획 원본
ap history                                              # 변경 이력
cat ~/.local/state/agent-progress/panes/*.json          # pane별 진행 창 상태
ap --plan <key> view                                    # 다른 pane에서 직접 보기
```

- **진행 창이 자동으로 안 열림**: `ap`가 stderr에 "Herdr pane을 확인할 수 없어…"를 출력했다면 pane을
  확인하지 못한 것입니다(공유 데몬이고 후보가 없거나 여럿). 안내된 `ap --plan <key> view`를 쓰거나
  `AP_PLAN`을 지정하세요. 닫음 표시가 남아 있으면 `ap open`으로 다시 엽니다.
- **진행 창이 오래된 상태라고 표시**: 계획 파일을 읽지 못한 것입니다. 진행 창은 마지막 정상 상태를 보여주고,
  CLI는 손상된 파일을 덮어쓰지 않습니다. 파일을 고치거나 `ap new`로 보관 후 새로 시작합니다.
- **줄이 밀리거나 `▀`가 보임**: 모호한 폭 문자를 두 칸으로 그리는 터미널 설정입니다. `AP_HALF_BLOCKS=0`으로
  반 칸 픽셀을 끕니다.
- **빛이 흐르지 않음**: 마지막 기록 후 `AP_IDLE_SECS`(기본 300초)가 지났거나, 100%이거나, 비어 있는
  계획입니다. 정상 동작입니다.

## 업그레이드와 이전 데이터

- 3.2 이전 계획 파일의 진행 창 필드(`viewer`, `view_suppressed`)는 읽을 때 무시되고 다시 쓰지 않습니다.
- 1.x·2.x의 `.agent-progress` 데이터(`*.json`, `v2/` 등)는 지우지 않지만 3.x가 읽지도 않습니다.
- 남아 있는 1.x 훅(`ap bridge` 등)이 있으면 실패합니다. 프로젝트 `.codex/config.toml`,
  `.claude/settings.local.json`, `.opencode/plugins/agent-progress.js`에서 제거하세요.
