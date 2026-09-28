# Codex 세션 → 체크리스트 → 별도 진행 상황 창

v0.3.0부터 제품 목표/세션 계획을 분리하고 native Plan 모드와 실제 두 세션의
연결을 검증했다. 최신 전체 계약은 `core-connection.md`를 우선한다.

## 연결

- 기본 `ap`: HERDR_PANE_ID의 위쪽 이웃을 명시적 `--pane`으로 조회한다.
  이 환경의 `--current`가 다른 포커스 pane을 반환한 실증 때문에 사용하지 않는다.
- 대상 agent가 Codex인지 확인하고 foreground native Codex PID 하나를 선택한다.
- `lsof -a -p PID -Fn`에서 그 프로세스가 실제로 연 rollout JSONL 하나만 선택한다.
  헤더의 UUID/cwd로 검증한다. 최신 파일/폴더/제목으로 세션을 추측하지 않는다.
- 실행 중 pane terminal ID, PID, 열린 rollout 경로를 다시 검사한다. 같은 PID의
  다른 세션 전환도 기존 계획에 조용히 섞지 않는다.
- `ap open`: 호출 pane 또는 명시한 pane 아래에 70/30 split, no-focus로 실행하고
  source pane/session을 인자로 고정한다. protected lobby에서는 거부한다.
- `--session`/`--rollout` 직접 읽기는 Herdr 활동 관측 없이 session event만 표시한다.

다른 에이전트를 실행하거나 새 Codex turn을 시작하지 않는다. 세션은 읽기 전용이다.
현재 어댑터는 macOS/Unix의 열린 파일 조회와 로컬 Codex 형식에 의존한다.

## 지원 입력

1. `response_item/function_call`의 `update_plan` 또는 `functions.update_plan`을
   같은 call_id의 `function_call_output = Plan updated`와 연결한다. 실패한 호출은
   반영하지 않는다. `pending/in_progress/completed` 상태를 읽는다.
2. `event_msg/plan_update`의 구조화된 step/status 목록을 읽는다.
3. native plan이 없는 세션: assistant의 `### 진행 계획`, `## 진행 계획`,
   `## Plan`, `### Plan` 아래 `목표:`/`Goal:`과 `- [ ]`, `- [>]`, `- [x]`
   목록을 읽는다. `event_msg/item_completed/AgentMessage`와
   `response_item/message role=assistant`의 중복 직렬화를 처리한다.
   일반 user message, tool output, reasoning에서는 체크리스트를 추출하지 않는다.
4. optional `goals_1.sqlite/thread_goals`에서 선택한 thread_id의
   goal_id/objective/status/created_at_ms만 SQLite READ_ONLY로 조회한다. 없으면 plan만으로
   동작한다. 지원하지 않는 스키마는 경고를 표시하며 plan 읽기는 유지한다.

native goal이 있으면 생성 시각 이전의 계획을 그 goal의 작업으로 가져오지
않는다. 새 goal에서 원본 plan이 아직 없으면 미정으로 표시한다. 외부 thread_id의
이벤트, 실패한 plan 호출, assistant의 코드 블록/인용 예시는 체크리스트에 넣지 않는다.
같은 목표에서는 native plan을 우선하되 명시적으로 목표가 바뀌면 이전 체크리스트를
보관하고 새 목표의 계획을 받는다.

v0.3.0 실제 검증: Codex 0.157.0에서 native goal 생성/변경/완료, native Plan
item, 일반 실행의 진행 보고, 두 세션과 서버 재시작까지 검증했다. 현재 실행
설정에는 update_plan tool이 없어 그 함수 호출의 live 증거로 일반화하지 않는다.

공식 [Codex App Server 문서](https://learn.chatgpt.com/docs/app-server)는
`turn/plan/updated`, `thread/goal/get`, plan item을 제공한다. 별도 app-server를
띄워 현재 TUI의 live 이벤트를 구독할 수 있다고 가정하지 않고, 이 버전에서는
현재 프로세스의 로컬 기록을 읽는 어댑터를 사용한다. 내부 형식 변경에 대응하는
버전별 adapter와 공식 transport 연결은 후속 범위다.

## 체크리스트와 저장

에이전트 plan 항목이 체크리스트 항목이 된다. 설명을 임의로 더 쪼개거나
완료 조건을 만들어 내지 않는다. 목표와 문구 정규화를 바탕으로 고정 ID를 만든다.
같은 문구가 다시 나타나면 기존 ID/완료 이력을 이어간다. 중복 문구는
식별이 모호하므로 오류로 처리하고 마지막 정상 계획을 유지한다.

- 체크율: 완료/전체. 항목 0개는 미정. native goal complete만으로 모든 항목을
  완료시키지 않는다. turn 종료도 항목 완료와 무관하다.
- 사라진 미완료는 계속 분모에 포함하며 `!` 표시. 사라진 완료도 이력과 함께
  목록에 남긴다. 문구 변경은 새 항목+이전 항목 누락으로 보수적으로 처리한다.
- 완료 → 진행으로 되돌리면 현재 완료 수는 줄이고 was_done은 보존한다.
- 목표 변경 시 이전 체크리스트는 archive에 보존한다. 새로운 목표에서만 새
  분모를 사용한다. UI 이력에서 이전 목표와 변경 내용을 확인한다.

읽기는 byte offset부터 증분 처리한다. 마지막 JSONL 줄이 부분 저장 중이면
newline이 생길 때까지 대기한다. 완성된 줄의 오류·삭제·교체·truncate는
마지막 정상 상태를 보존한다. 원본 전체를 복제하거나 로그를 출력하지 않는다.

`.agent-progress/<session>.json` 체크포인트에는 파생 목표/항목/변경 이력,
source identity/offset, 처리 중인 plan 호출 정보만 저장한다. 원자적 교체와
advisory lock과 읽었던 cache bytes의 비교를 사용한다. 같은 offset이어도 다른
창이 먼저 쓴 기록은 덮어쓰지 않는다. 다른 창과의 지속 경합은 해당 창을 정리한
뒤 다시 연결하거나 별도 `--cache`를 사용해야 한다.

source의 이미 읽은 prefix fingerprint와 checkpoint 내용 checksum으로 우발적
수정/손상을 감지한다. 이는 인증/서명이 아니다. 파일 메타데이터 변경 시 prefix를
다시 검증하므로 큰 세션은 더 많은 I/O가 필요하다. 읽기·검증·저장·Herdr 조회는
UI 입력과 분리된 worker에서 수행하며 종료 시 취소하고 조회 프로세스를 회수한다.
저장 실패는 성공할 때까지 경고를 유지하고 새 원본 이벤트 없이도 재시도한다.

raw 대화·토큰·인증 정보를 복사하지 않는다. 한 record/checkpoint는 16 MiB로
제한하되 전체 rollout이 더 크면 여러 chunk를 읽는다. `--once`도 뒤의 최신 계획을
누락하지 않는다. v0.2 checkpoint는 `.legacy-<UUID>.json`으로 보존한 뒤 원본을
다시 읽어 새 형식으로 저장한다. 이전 기록 중 원본에서 복원할 수 없는 데이터는
백업에 남는다. 캐시 손상은 자동 삭제하지 않는다. 원본 데이터 복구 도구는 아니다.

## 아직 남은 범위

Claude Code/OpenCode 어댑터, 문구 변경의 명시적 ID 매핑, 세션 간 목표 연결,
원본 스키마 변경 호환, 직접 편집 재대조, 백업/복원, 자동 검사/사람 근거와의
연결, 접근성 사용자 검증, 배포/업데이트. 전체 범위는 제품 문서를 유지한다.
