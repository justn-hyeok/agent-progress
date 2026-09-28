# 핵심 연결 계약 — v0.3.0

## 목표와 소유권

`ap.project.json`은 제품 목표 UUID, 이름, 기존 제품 계획과 요구 검증 수준을
선언한다. task를 다시 등록하는 DB가 아니다. 현재 repo는 원래 작성한
`docs/product-completion-plan.md`의 AP-01~AP-24를 그대로 사용한다.

- 제품 목표 ID: 선언의 `project_id`. 세션 ID/cwd로 매번 새 목표를 만들지 않는다.
  선언 하나는 제품 목표 하나이며, 다른 제품은 다른 UUID로 구분한다.
- 작업 ID: 제품 UUID + 원본의 AP ID. 이름·순서·세션 변경으로 새 작업이 되지 않는다.
- 세션 goal/plan: Codex 원본을 읽은 `Feed/Snapshot`. 제품 목표를 교체하지 않는다.
- 공통 도메인: 제품과 파일 모드가 같은 `model::Plan/Task`를 사용한다. 상태,
  조건, 검증 종류, 근거, 연결 세션, revision/이력을 같은 규칙으로 검증한다.
- 화면: 제품 전체 완료 수와 현재 세션의 세부 작업을 조합한다. `overall` 필드에
  두 범위를 분리하므로 세션 3/3이 제품 24/24가 되지 않는다.

원본 native 세션·인증은 읽기 전용이다. 제품 상태는
`.agent-progress/project-<UUID>.json`에 저장한다. writer가 잠금 아래 최신 상태를
읽고 원자적으로 저장한다. 기존 사용자 파일과 세션 checkpoint는 보존한다.

## 자동 연결과 상태 규칙

정확한 Herdr pane/PID/열린 rollout → 그 세션 cwd의 가장 가까운 선언을 찾는다.
선언이 없으면 기존 독립 세션 모드다. 다른 프로젝트 cwd는 거부하며, 현재
포커스나 최신 파일을 근거로 세션을 선택하지 않는다.

1. `[AP-05] ...` 또는 `AP-05 ...`: 해당 제품 항목 전체의 직접 보고.
2. `[AP-05/native] ...`: 세부 단계. 진행 중이면 상위는 진행, 자식들만 끝나면
   상위는 검증 대기다. 자식 완료만으로 제품 완료 수를 늘리지 않는다.
3. 원본과 완전히 같은 유일한 제목은 연결 가능하다. 그 밖의 모호한 입력은
   현재 작업으로만 표시하고 제품 완료율에 반영하지 않는다. 상세에 미연결 표시.
4. 같은 제품 ID를 여러 번 직접 주장하면 임의로 하나를 선택하지 않는다.

같은 입력은 중복 반영하지 않는다. 이전 시각의 세션은 최신 완료를 되돌리지 않는다.
새 세션의 기본 pending 목록도 완료 항목을 자동 재개하지 않는다. 명시적 active,
문서 체크 전이, 제품 상태 명령으로 재개할 수 있다. 누락은 자동 취소가 아니며
이전 항목을 분모와 이력에 유지한다. 프로젝트 이동 시 선언과 상태를 함께 옮긴다.

퍼센트는 취소를 제외한 공통 Task의 완료/전체다. native goal complete나 turn 종료는
제품 전체 완료 근거가 아니다. 원본 completed는 에이전트 보고로 기록하며 요구
검증을 충족하지 못하면 검증 대기다. AP-05는 automated, AP-24는 human을 요구한다.
별도 세부 조건이 없는 입력은 원본 항목 제목을 기본 조건으로 가져오며, 자세한
수용 조건은 원본 제품 문서에서 확인한다.

## 에이전트용 공통 명령

일반 사용자의 재등록 절차가 아니라, 에이전트가 같은 도메인에 근거/판정을
명시적으로 기록하는 인터페이스다.

```sh
ap product show
ap --actor codex product evidence AP-05 automated 'docs/evidence/native-contract.json'
ap --actor codex product status AP-05 done --reason '실제 native 입력과 재시작 검증 통과'
```

다른 선언은 `ap product --project <manifest> ...`. `--file`/`--expect-revision`은
파일 모드용이다. evidence는 기록이며 검사 실행/사람 인증을 대신하지 않는다.

## 실제 Codex 계약

검증 환경: **Codex 0.157.0 / gpt-6-sol / 기본 host 설정**.

- 실제 `thread/goal/set/get` 생성·변경·완료·서버 재시작 후 보존을 확인했다.
- Plan 모드는 native `item/completed: plan` 및 rollout `item_completed: Plan`을
  기록한다. 실제 이벤트를 읽어 체크리스트를 만들었다.
- 이 설치본의 일반 실행에는 `update_plan` tool이 노출되지 않았다. native Plan
  이후 일반 실행의 명시적 `진행 계획` 보고로 상태를 갱신하는 경로를 실제 검증했다.
  출처를 `Codex native Plan · 진행 보고`로 구별한다.
- `update_plan`/`plan_update` 파서는 유지하지만 이 설정에서 해당 tool을 실제
  실행했다고 주장하지 않는다. native Plan 모드와 구별한다.
- native Plan의 checkbox/번호 목록을 지원하고 코드 예시/인용문은 제외한다.
  자유 서술만 있는 계획은 경고와 마지막 상태를 유지하며 다음 입력을 기다린다.
- Goal DB가 없어도 제품 문서와 계획으로 작동한다. native goal 보강 입력만
  정확한 thread의 goal 저장소를 읽는다. 다른 하네스 완료를 의미하지 않는다.

공식 [App Server 계약](https://learn.chatgpt.com/docs/app-server), 설치본 생성
schema, 실제 rollout을 대조했다. 별도 서버가 현재 TUI를 자동 구독한다고
가정하지 않고, 현재 프로세스의 정확한 로컬 세션을 읽는다.

## 검증과 후속 범위

- `tests/project.rs`: 세션 교체/stale reader/제품 이동/이름·누락/자식 완료/
  검증 정책/원본 오류/중복 ID/다른 프로젝트 차단.
- `scripts/pty_product.py`: 실제 TUI에서 제품 50%/자식 100% 구별, 현재 작업,
  상세/리사이즈/종료 복원.
- `scripts/native_contract.py`: 실제 Codex 테스트 세션 두 개, native goal/Plan과
  일반 실행 보고, 0/2→1/2→새 세션 1/2→2/2, 동일 ID, stale session 재읽기,
  goal 변경·완료·서버 재시작 검증. 모델은 제공된 테스트 상태만 출력하며
  코딩/파일 작업을 위임하지 않는다. 기존 사용자 세션/설정은 변경하지 않는다.

후속 범위는 다중 제품 선택 UX, 의미가 바뀌는 분할/병합, 조건별 검증 및 revision
freshness, 복원 UX, Claude/OpenCode/MCP, 설치 배포와 사람의 최종 제품 QA다.
핵심 연결 완료를 제품 전체 완료로 계산하지 않는다.
