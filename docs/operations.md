# 세 에이전트 제품 사용과 복구

## 1.2.3 중첩 프로젝트의 계획 경계

`ap.project.json`이 없는 프로젝트에서 native 훅이 입력을 받으면 작은 passive-root
표시를 `.agent-progress/`에 남긴다. 명시적으로 연결된 설정, 이전 연결 영수증,
별도 Git 루트도 상위 프로젝트의 제품 선언을 암묵적으로 상속하지 않는 경계다.
해당 디렉터리에 유효한 제품 선언이 있으면 그 선언이 우선한다. 표시가 변경되거나
심볼릭 링크로 바뀌면 다른 프로젝트를 골라 연결하는 대신 오류로 남긴다.
연결 설정 제거 후에도 기존 세션 기록의 경계를 지키므로 표시를 자동 삭제하지 않는다.
동시 첫 이벤트는 같은 표시를 공유하며, 표시 내용이 훼손되면 상위 제품으로
넘어가지 않고 오류를 보고한다.

v1.2.2에서 하위 일반 프로젝트의 진행이 상위 제품 기록에 반영되었을 수 있다.
이미 저장된 상위 제품 기록은 자동 복구하지 않는다. 작업별 출처와 근거를 확인하고
기존 백업·명시적 복원 기능으로 처리해야 한다. 확인 전 완료 근거를 임의로 바꾸지 않는다.

## 1.2.2 일반 프로젝트 연결과 진단

`ap connect preview --agent NAME`은 기존 출력에 `connection_status`,
`next_action`, 정적인 `diagnostic`을 추가한다. unmanaged/apply는 새 연결 가능,
managed/none은 기존 연결 유지, conflict/inspect는 사용자 수정·미완료 설치·소유권
없는 설정·잘못된 경로를 먼저 점검하라는 뜻이다. preview는 파일을 만들지 않고
사용자 설정 전문을 출력하지 않는다. apply/remove의 보호 동작은 유지한다.

일반 프로젝트에는 manifest가 필요 없다. Codex는 passive hooks만 설치하며 기존
유효한 ap.project.json이 있을 때 product MCP를 추가한다. Claude/OpenCode 훅도
manifest 없이 native 계획을 읽는다. 선언·goal·roadmap을 임의 생성하지 않는다.
잘못된 선언과 다른 root를 가리키는 선언은 오류로 남긴다. product write MCP는
기존 manifest가 필요하다. AP_AUTO_OPEN=0은 실행기에서 덮어쓰지 않는다.

`ap doctor --agent NAME`은 native CLI 설치/버전, 현재 프로젝트 연결 소유권,
선택적 제품 선언·캐시 무결성, 현재 셸 설정, tmux 가용성을 점검한다. 제품
선언이나 사용하지 않은 셸 연결이 없는 것은 기본적으로 정보 상태다. 손상된
manifest·roadmap·캐시나 연결 충돌은 건강하지 않은 상태로 표시한다. `--pane`,
`--rollout`, `--terminal-slot`은 명시한 원본만 읽고 최신 후보를 추측하지 않는다.
기본 종료 코드는 보고용 0으로 유지한다. 선택한 검사를 자동화에서 통과 조건으로
삼을 때 `--strict`를 사용하며, `healthy:false`이면 JSON을 출력한 뒤 실패한다.
`--shell NAME --rc PATH`는 해당 rc 파일만 점검한다. 자동 셸 검사에서 absent·partial은
안내로 남기고, explicit 셸 검사에서는 current만 통과한다. 모든 검사는 원시 설정
내용을 출력하지 않으며 파일·훅·로그인·사용자 터미널을 수정하지 않는다.

## 1.2.1 bash·fish, 일반 터미널, 호환성 검사

셸 설정에는 `--shell bash` 또는 `--shell fish`를 지정한다. bash 기본 설치는
`.bashrc`와 기존 활성 로그인 파일에 같은 보호 블록을 설치하고 각각 백업한다.
`.bash_profile` → `.bash_login` → `.profile` 순서를 보존하고, 없으면 `.profile`을 사용한다.
새 파일로 이전 설정을 가리지 않으며, 설치 뒤 우선순위가 바뀌어도 원래 관리 블록을 찾아 제거한다.
fish는 XDG_CONFIG_HOME/fish/config.fish를 사용한다. preview는 파일을 만들지 않는다.
status/remove에도 같은 셸을 지정하며 `--rc PATH`는 파일 하나만 처리한다.
기존 별칭·함수, 편집된 관리 블록, 원래 실행 파일은 보존한다.

Herdr 밖의 대화형 `ap launch --agent NAME`은 선택적 tmux로 진행 창을 자동 생성한다.
tmux가 없으면 설치 방법을 알리고 원래 명령을 실행한다. 기존 tmux에서는 별도
작업 window를 사용하고, 없으면 전용 socket의 세션을 현재 터미널에 연결한다.
에이전트 쪽 포커스와 초기 above/below 설정을 유지한다. 비대화형·관리 명령은
창을 생성하지 않는다. AP_AUTO_OPEN=0 또는 auto_open:false로 끌 수 있다.
Codex는 해당 frontend RPC 응답을 읽고 Claude/OpenCode는 연결된 프로젝트 훅의
실제 부모 프로세스를 검사한다. 최신 로그 추측이나 다른 창의 식별자는 쓰지 않는다.
원본 종료 후 viewer와 실행별 임시 정보가 정리되며 native 종료 코드를 보존한다.

실행기는 버전 변경 시 로컬 CLI와 Codex RPC 스키마를 재검사한다.
결과는 .agent-progress/compatibility/에 저장하고 실패하면 경고하되 native 실행을 유지한다.
`ap compatibility`는 같은 검사를 JSON/종료 코드로 반환한다. `--agent NAME --live`는
기존 인증으로 짧은 모델 응답을 검사하며 사용량이 발생한다. 필요하면 --model을 지정한다.
실제 markdown 계획이 HUD의 1/2 상태로 전달되는지 검사한다. native TaskCreate/todos,
전체 창 lifecycle, 다른 버전의 모든 기능은 별도 검증이다. 원시 provider 오류 로그는 출력하지 않는다.

v1.2.0 사용자는 바이너리 업그레이드 후 `ap shell install --shell zsh`로 기존 블록을 갱신한다.
기존 블록은 백업하고, 사용자가 편집한 블록은 자동 덮어쓰지 않는다.

## 1.2.0 일반 codex 명령 연결

`ap shell preview`는 zsh 설정에 추가할 블록을 보여줍니다. `ap shell install`은
`ZDOTDIR`이 있으면 그 디렉터리, 아니면 HOME의 `.zshrc`를 백업하고 관리 블록을
추가합니다. `--rc /명시적/파일`로 대상을 지정할 수 있습니다. 심볼릭 링크 파일,
중복·불완전하거나 편집된 관리 블록은 보존하고 거부합니다. 설치는 멱등적입니다.

새 셸에서 `codex`·`codex resume`는 Herdr 안에서 `ap launch`로 전달합니다.
원래 실행 파일은 교체하지 않으며, Rust 실행기가 셸 함수를 다시 부르지 않아
재귀가 발생하지 않습니다. Herdr 밖, ap가 없는 PATH, 비대화형·관리 하위 명령은
원래 Codex로 실행합니다. 기존 별칭·함수는 우선합니다. native 프로젝트 신뢰나
로그인 요구를 우회하지 않습니다.

`ap shell remove`는 정확한 관리 블록만 제거해 다른 사용자 편집을 유지합니다.
새 셸에 적용되며 현재 셸에서 이미 로드한 함수는 `unfunction codex`로 지울 수
있습니다. 원래 실행을 즉시 호출하려면 `command codex`를 사용합니다.

## 1.1.0 YAML·프리셋·배경

`ap config presets`로 Signal/Forest/Ocean/Amber와 사용자 프리셋을 확인한다.
`ap config set --preset forest --brightness 1.2`로 배경을 밝게 만들 수 있다.
프리셋 변경은 기존 개별 색 덮어쓰기를 해제한다. 밝기는 0.25~2.0이며
track/fill에만 적용한다. 기본 밝기는 1.0이다.

`ap config init-yaml`은 현재 색을 보존해서 `.agent-progress/ui.yaml`을 만든다.
YAML이 있으면 기존 `ui.json`보다 우선한다. [예제](theme-example.yaml)를 참고해
`presets`에서 다른 프리셋을 `extends`하고 `theme`으로 색을 덮어쓴다.
`states`는 working/waiting/blocked/paused/error/done/empty 별 색을 설정한다.
색상 선택은 표시된 상태를 따르며 완료 비율이나 계획을 수정하지 않는다.
YAML 앵커·merge를 지원한다. CLI로 YAML을 저장하면 주석과 서식은 다시 작성된다.
잘못된 설정은 실행 중 창의 마지막 유효 색을 유지한다.

핵심 파일 모드, 명시적 rollout 읽기, MCP, 설정은 Herdr 없이 실행된다.
Herdr는 pane 생성·이동과 자동 원본/활동 식별에만 필요하다.

## 1.0.1 테마·자동 호출·위치

연결된 native 훅은 Herdr 안에서 원본을 확정하면 진행 창을 자동으로 호출한다.
`ap launch`/AP_AUTO_OPEN=1은 자동 호출의 선행 조건이 아니다. 기존 Codex 0.157.1
embedded/proven-owner 조건과 protected lobby 보호는 유지한다. Herdr 밖에서는
창을 만들지 않는다. `ap config set --auto-open false` 또는 AP_AUTO_OPEN=0으로 끈다.

`ap config set --position above`는 위, `--position below`는 아래를 기본 배치로
저장한다. `ap open --position above`는 이번 호출만 덮어쓴다. 같은 관리 창의
직접 수직 형제 배치라면 창을 교환하고 원래 크기/포커스를 유지한다. 모호하거나
다른 구조로 이동한 창은 임의 조작하지 않는다. 위에 있는 viewer도 바로 아래의
agent를 찾으며 양쪽에 agent가 있으면 명시적 --pane을 요구한다.

색 예: `ap config set --accent '#88AAFF' --fill '#1E293B' --track '#0F172A'`.
설정 역할은 track/fill/accent/text/muted/metadata/warning이고 모든 값은 #RRGGBB다.
`ap config show`로 조회, `ap config reset`으로 기본 Signal/아래/자동 호출을 복원한다.
실행 중 창에도 색이 반영되며 불완전한 직접 편집에서는 이전 색을 유지한다.
설정 파일은 `.agent-progress/ui.json`이고 goal/계획/에이전트 전역 설정과 별개다.

기본 흐름은 기존 Codex 작업을 별도 진행 창에서 따라가는 것이다. 수동 작업
등록, UUID 복사, 별도 동기화는 필요 없다. 아래 명령은 에이전트/진단용 탈출구다.
제품 전체 수용 기준은 `product-completion-plan.md`이며 단계 완료와 구별한다.

## 연결과 진단

`ap open`은 정확한 호출 Codex/Claude/OpenCode pane 아래에 포커스를 바꾸지 않고 창을 연다.
같은 source/session의 활성 창은 재사용한다. 실행을 끝낸 관리된 shell은
재연결에 사용하며, 다른 프로세스가 있으면 입력을 보내지 않는다.

`ap doctor --pane SOURCE`는 버전, 선언/저장 상태, 원본 경로, 실제 이벤트
수신과 복구 방법을 JSON으로 출력한다. 원본이나 설정을 수정하지 않는다.
닫힌 창의 기록은 `ap open --pane SOURCE --reconnect`로 live inventory에서
해당 terminal이 사라진 것을 확인한 후 복구한다. 다른 source/세션, 이동한
terminal, 읽을 수 없는 inventory는 임의 추측으로 연결하지 않는다.

Codex 0.157.1/Herdr 0.9.0에서 열린 rollout과 native hook의 정확한 transcript
등록을 지원한다. 훅은 source 프로세스의 실제 자식인지 확인한 뒤
pane/terminal/PID/session을 등록한다. 공유 daemon의 상속 환경변수만으로는
다른 client를 구별할 수 없으므로 그 훅의 pane 등록을 거부한다. 식별 근거가 없으면 연결 불가로 진단하며
최신 파일/추측 UUID를 쓰지 않는다. `--reconnect`는 같은 source terminal의
새 세션으로 넘어갈 때 관리된 진행 창을 종료·재사용하고 이전 receipt는 보존한다.

## 프로젝트 연결과 native 입력

`ap connect preview` 뒤 `ap connect apply`는 프로젝트의 Codex MCP/훅과 AGENTS
추가분만 관리한다. `--agent claude`는 `.claude/settings.local.json`,
`--agent opencode`는 `.opencode/plugins/agent-progress.js`를 같은 방식으로 관리한다.
`ap connect remove --agent NAME`은 정확한 관리 부분만 제거하고 backup을 보존한다.
사용자 수정/같은 이름의 무관한 plugin은 덮어쓰지 않는다. 전역 설정은 바꾸지 않는다.

1.1.1의 `ap launch --agent codex`는 설치된 native Codex의 연결을 관찰해 실행한 창과
실제 세션을 짝짓는다. 기본 실행에서는 공유 서버를 끄지 않는다. 응답의 세션 ID와
native 프로세스 소유권을 확인하고, 다른 세션 조회나 늦은 응답을 현재 세션으로
오인하지 않는다. 원시 전송 내용은 바꾸거나 저장하지 않는다.
Codex 인자는 `ap launch --agent codex -- resume SESSION`처럼 전달한다.
`--cd`의 실제 디렉터리를 적용한다. native CLI가 embedded 실행을 요구하는 명시적
설정 옵션과 `--no-daemon`은 사용자 선택 그대로 전달한다. 전역 설정은 변경하지 않는다.
등록에 성공하면 런처의 진행 창은 자동으로 열리며 같은 source terminal의 다음
세션에서는 관리된 창을 재사용한다. protected lobby/busy/unknown pane은 건드리지
않고 진단만 남긴다. 원본의 공유 서버 세션을 그대로 읽을 때는 정확한 Herdr/owner metadata가
있는 기존 경로를 지원하며 native hook의 자동 식별 증거와 구별한다.

Codex 프로젝트는 먼저 Codex에서 신뢰된 상태여야 한다. native hook review도
Codex가 요구하면 적용한 명령을 확인한다. 비 Git 프로젝트에서는 Codex의
`project_root_markers`에 `ap.project.json`이 있거나 현재 디렉터리를 root로 쓰는
설정이 필요할 수 있다. native 프로젝트 설정과 신뢰 조건은 해당 하네스를 따른다.
진행 창은 에이전트 입력/대화를 바꾸지 않는다. 에이전트는 기존 계획을 유지하며
일반 실행 보고는 `### 진행 계획`과 체크리스트로 표시하면 자동으로 반영된다.

Claude는 native TodoWrite/TaskCreate/TaskUpdate와 명시적 Stop 진행 보고를,
OpenCode는 native todo.updated와 assistant의 명시적 진행 보고를 변환한다.
하네스별 native goal이 없으면 만들어내지 않는다. raw transcript/임의 prose는
복사하지 않고 세션 식별과 명시적 계획만 저장한다. 원본의 완료는 보고이며
제품 수용 조건/자동 검사/사람 확인은 별도다. `ap product summary`는 같은
복귀 정보를 일반 텍스트로 제공한다. 스크린리더 인증을 의미하지 않는다.

## 공통 모델과 계획 변경

파일과 제품은 같은 Plan/Task, 검증 종류, 완료 조건, 선행 작업 규칙을 쓴다.
`status`는 파일 모드의 엄격한 명시적 판정이고 `report`는 완료 보고를 저장하되
요구 근거가 부족하면 검증 대기로 유지한다. 제품의 source/상태 보고는 후자의
정책을 사용한다. 프로세스 종료, goal complete, 자식 단계 완료는 제품 완료가 아니다.

`ap.project.json`의 선택적 `tasks`는 고정 ID별 `criteria`, `depends_on`,
`revision_paths`를 제공한다. 생략된 조건을 임의로 추가하지 않는다. Root ID는
세션/이름/순서와 독립적이고 명시적 세부 ID도 이름 변경에서 유지된다.

`ap product replace --from AP-01 --into AP-02 AP-03 --reason '분할 이유'`는 이미
선언된 ID 사이의 명시적 분할/병합이다. 원본을 취소하고 lineage/이력을 보존하며
새 완료나 근거를 상속하지 않는다. 선행 작업 의미가 모호하면 자동 수정하지 않고
거부한다. 누락은 취소가 아니며 분모를 보존한다. 의미/완료 조건의 축소는
별도 사용자 결정이 필요하다.

## 검증 근거와 복귀

선택적 `revision_paths`는 프로젝트 안의 구체적인 코드 파일만 지정한다.
검사 전에 `ap product revision AP-01`로 지문을 얻고, 통과 후
`ap product evidence AP-01 automated result.json --revision HASH`로 기록한다.
코드가 달라지면 기록을 거부한다. 관측된 코드 변경은 기존 근거를 오래됨으로
표시하고 완료를 재검증으로, 종속 작업을 막힘으로 돌린다.

근거는 결과/기록자의 선언이며 앱이 검사 실행, 사람 인증, 서명을 대신하지 않는다.
`product invalidate ID --reason 이유`, `product note ID 메모 --next 다음행동`은
같은 도메인에 기록된다. `product resume`는 최근 완료, 변경, 막힘, 다음 실행 가능한
작업과 원래 근거를 보여준다. `ap projects --project MANIFEST1 MANIFEST2`는 명시한
저장 목표를 읽기 전용으로 전환한다. 기본 follow는 현재 source의 제품을 자동 선택한다.

## 저장, 백업, 이전

공통 Plan/제품 저장 형식은 v2다. v1을 검증해 읽고 최초 변경 전에 정확한 v1
백업을 보존한다. 최신 이전 정상 제품 상태는 `project-UUID.previous.json`이다.
제품 JSON checksum은 우발적 손상을 감지하며 서명/인증이 아니다. symlink 저장,
중복 ID, 잘못된 관계, 손상, writer 경합은 덮어쓰지 않는다.

`product backup --output FILE`은 원본 bytes를 복제한다. 기존 파일을 덮어쓰지 않는다.
`product restore --backup FILE`은 미리보기와 `current_hash`만 출력한다. 같은 명령에
`--expect-hash HASH`를 넣으면 잠금/충돌 확인 후 복원하고 기존 bytes는 displaced
파일로 보존한다. 손상된 현재 데이터도 삭제하지 않는다. 복원 후 지정 코드 입력을
재대조해 낡은 근거를 다시 판정한다.

`product export --output BUNDLE`과 `ap import --input BUNDLE --into NEW_DIRECTORY`는
선언/roadmap/공통 상태를 이동한다. raw 대화/인증은 포함하지 않는다. import는
기존 디렉터리를 거부하며 중단된 import marker가 있으면 부분 결과를 정상 연결하지
않는다. 원본 bundle은 보존하고 새 빈 목적지로 다시 실행한다.

최초 배포 후보는 v0.4.0이다. v2를 지원하는 패키지끼리의 업데이트/롤백에서 기록
유지를 검사한다. 개발용 v0.3으로 새 v2 데이터를 직접 읽는 downgrade는 지원하지
않는다. 원래 v1 백업은 유지되며, 신형 기록과 백업을 보존한 뒤 호환 도구로 읽는다.

## 선택적 MCP/지침

`ap mcp --project MANIFEST`는 로컬 stdio, 기본 읽기 전용이다. `--allow-writes`로
상태/메모/근거 도구를 명시적으로 노출한다. handshake 전 호출과 알 수 없는
인자는 거부한다. 전용 서버/네트워크/goal DB를 필요로 하지 않는다.

`ap connect preview`는 관리할 추가 내용만 표시한다. `connect apply`는 현재
프로젝트의 `.codex/config.toml`과 `AGENTS.md`만 private backup 후 병합한다.
`connect remove`는 정확한 관리 블록만 제거하고 이후 사용자 편집을 유지한다.
관리 블록이 달라지거나 무관한 같은 이름의 MCP가 있으면 보존하고 거부한다.
사용자 전역 설정은 수정하지 않는다. Codex의 프로젝트 신뢰/재시작이 필요하며
현재 agent-progress 프로젝트에도 사용자가 승인한 범위로 적용했다.
설치가 이를 필수 절차로 만들지 않는다.

프로토콜 근거: [MCP stdio](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports),
[MCP lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle),
[Codex project MCP](https://learn.chatgpt.com/docs/extend/mcp).

## 검증 범위

자동 셀/PTY, 장애 주입, 실제 native producer와 설치 prefix 검증은 사람 QA와
구별한다. `AP_FAULT_STAGE`는 private fault fixture 전용이며 실제 사용에서 설정하지 않는다.
release의 10/100/500 MiB 증분 표시/종료를 측정한다. debug 500 MiB는 느린 별도
개발 환경이다. 지원 OS/서명/설치 범위는 `distribution.md`를 따른다.
개인 세션 원문과 사용자 화면 증거는 로컬에만 보존한다. 공개 저장소에는
재현 가능한 테스트와 GitHub CI 결과를 제공한다.
