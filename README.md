# agent-progress

**에이전트가 지금 무엇을 하고 있고, 다음에 무엇을 할지 한눈에 보는 터미널 진행 창.**

Codex·Claude Code·OpenCode의 기존 목표와 계획을 읽어 Herdr의 별도 창에 표시합니다. 작업을 다시 등록하거나 세션 ID를 복사할 필요 없이, 평소 쓰던 에이전트의 계획을 따라갑니다.

[다운로드](https://github.com/justn-hyeok/agent-progress/releases/latest) · [에이전트 스킬](skills/ap/SKILL.md) · [운영·복구](docs/operations.md) · [변경 이력](CHANGELOG.md)

## 설치

현재 배포 패키지는 **macOS Apple Silicon용**입니다. Homebrew로 설치합니다.

```sh
brew tap justn-hyeok/agent-progress https://github.com/justn-hyeok/agent-progress
brew install justn-hyeok/agent-progress/agent-progress
ap --version
```

Rust·Node·Herdr는 패키지 설치 의존성이 아닙니다. 진행 창 자동 생성과 위치 이동에는 Herdr가 필요합니다. Homebrew 대신 릴리즈의 `.tar.gz`와 체크섬을 받아 설치할 수도 있습니다. [패키지 설치 방법](docs/distribution.md)을 참고하세요.

설치 후 진행 창을 쓸 프로젝트 루트에서 `ap setup`을 실행하면 높이를
Herdr 최소(10%)·작게(20%)·보통(30%)·크게(40%) 또는 10~50% 사이의 값으로 고릅니다.
Homebrew 설치 자체는 대화형 설정을 실행하지 않습니다. 설정하지 않으면
Herdr 최소 높이인 10%를 사용합니다. 빈 에이전트 세션에서는 진행 창을 열지 않고,
해당 세션에 goal 또는 plan이 생기면 자동으로 엽니다. 기존에 저장한 크기는
유지하며, Herdr에서 손으로 조절한 높이는 자동 재연결이 덮어쓰지 않습니다.
`ap config set --pane-size`를 실행하면 저장된 크기를 즉시 적용합니다.
이 설정은 프로젝트별로 저장되며 목표·계획은 바꾸지 않습니다.

```sh
cd /path/to/project
ap setup
```

```sh
# 업데이트
brew update
brew upgrade agent-progress

# 제거 — 프로젝트 데이터는 보존
brew uninstall agent-progress
```

## 에이전트 연결

v1.2.4는 연결 설정이 잠깐 잠겨 있을 때 제한된 시간 안에서 재시도합니다.
설정 내용이 바뀌었는지는 잠금을 얻은 뒤 다시 확인하므로 사용자 편집은 덮어쓰지 않습니다.

v1.2.3은 선언 파일이 없는 하위 프로젝트의 계획이 상위 제품 계획에 섞이지 않도록
연결 경계를 보존합니다. 기존 제품 선언이 해당 프로젝트 루트에 있으면 제품 계획을
그대로 읽습니다. 과거 v1.2.2에서 하위 프로젝트를 연결해 사용했다면 상위 제품의
진행 기록을 확인하세요. 기존 기록은 자동으로 되돌리지 않습니다.

v1.2.2는 일반 프로젝트 연결과 확장된 진단을 지원합니다. v1.2.1부터
bash·fish, Herdr 밖 자동 창, 버전 변경 시 호환성 검사를 지원합니다.
v1.2.0에서 업그레이드했다면 `ap shell install`을 다시 실행해 기존 관리 블록을 갱신합니다.

zsh·bash·fish에서 평소처럼 `codex`를 입력해 자동 연결하려면 사용하는 셸에 한 번 설치합니다.

```sh
ap shell install
ap shell install --shell bash
ap shell install --shell fish
```

새 터미널에서 `codex` 또는 `codex resume`를 사용하면 됩니다. Herdr 안에서는
연결 관찰기를 거쳐 원래 Codex를 실행합니다. Herdr 밖의 대화형 터미널에서는
tmux로 에이전트와 진행 창을 자동 표시합니다. 이 경로에는 `brew install tmux`가
필요합니다. 기존 tmux 안에서는 별도 작업 window를 열어 다른 창의 배치를 보존합니다.
`codex exec`, `login`, `mcp` 같은 비대화형·관리 명령도 원래 Codex로 직접 전달합니다.
`ap shell status`로 최신 상태·업그레이드 필요·없음·부분 설정·충돌을 확인하고,
`ap shell remove`로 제거합니다. 알려진 구형 블록만 업그레이드 대상으로 표시합니다. 기존 rc 파일은
백업하며 사용자 편집과 원래 실행 파일은 보존합니다. 기존 `codex` 별칭·함수는
덮어쓰지 않습니다. 앱 제거 전 이 설정도 제거하세요.

`status`·`remove`에도 설치할 때와 같은 `--shell`을 사용합니다. bash는 기본적으로
`.bashrc`와 기존 로그인 파일(`.bash_profile`, `.bash_login`, `.profile` 중 우선하는 파일)을
처리해 일반·로그인 셸을 지원합니다. 로그인 파일이 없으면 `.profile`을 사용합니다. fish는
`$XDG_CONFIG_HOME/fish/config.fish`(기본 `~/.config/fish/config.fish`)를 처리합니다.
`--rc PATH`를 지정하면 그 파일 하나만 처리합니다.

셸을 거치지 않는 직접 프로그램 호출은 이 함수의 대상이 아닙니다. 원래 실행이
필요할 때는 `command codex`를 사용합니다. 비대화형 실행과 tmux가 없는 환경은
원래 에이전트로 전달합니다. `AP_AUTO_OPEN=0`으로 자동 창 생성을 끌 수 있습니다.

작업할 프로젝트로 이동한 뒤, 사용할 에이전트를 연결합니다. Codex의 경우:

```sh
ap connect preview --agent codex
ap connect apply --agent codex
ap launch --agent codex
```

`preview`는 적용 내용을 보여주고, `apply`는 프로젝트의 연결 설정을 백업한 뒤 추가합니다. `launch`는 설치된 Codex를 실행하고, 그 창의 실제 세션 시작·복귀 응답을 읽어 진행 창을 연결합니다. 기존 공유 서버를 끄거나 세션 ID를 입력할 필요가 없습니다. 프로젝트 신뢰나 훅 승인이 요구되면 해당 에이전트에서 확인하세요.

일반 프로젝트에서 `ap.project.json` 없이 연결할 수 있습니다.
기존 manifest가 있으면 제품 계획·Codex product MCP를 유지하고, 잘못된 선언은 오류로
표시합니다. `preview`에 추가된 `connection_status`는 `unmanaged`·`managed`·`conflict`이며,
`next_action`은 각각 `apply`·`none`·`inspect`입니다. 이미 관리 중인 설정은 유지하고,
충돌은 사용자 편집을 보존한 채 점검합니다.

Herdr에서는 연결된 훅이 원본 세션을 확인하면 진행 창을 자동으로 엽니다. Herdr 밖에서는
`ap launch --agent NAME`이 진행 창을 먼저 열고, 해당 실행의 실제 계획을 기다립니다.
Claude·OpenCode에는 프로젝트 연결 훅이 필요합니다. 에이전트는 기존 계획 도구를
그대로 사용합니다. Herdr에서는 `ap open`으로 직접 열 수도 있습니다.

| 에이전트 | 연결 명령 | 읽는 계획 |
| --- | --- | --- |
| Codex | `ap connect apply --agent codex` | Native Plan·명시적 진행 보고·선택적 goal |
| Claude Code | `ap connect apply --agent claude` | TodoWrite·TaskCreate/TaskUpdate·명시적 진행 보고 |
| OpenCode | `ap connect apply --agent opencode` | Native todos·명시적 진행 보고 |

Claude Code와 OpenCode도 설정 적용 후 훅·플러그인을 읽는 세션에서 작업합니다. 연결 설정은 프로젝트 안에서만 관리하며 에이전트 전역 설정은 바꾸지 않습니다. [연결 조건과 진단](docs/operations.md)을 참고하세요.

## 에이전트 스킬

에이전트에게 진행 창·연결·테마·복구를 맡기려면 필요한 스킬을 설치하세요. **앱의 Homebrew 설치와 스킬 설치는 별개입니다.** `$ap`는 기존 호출을 유지하는 기본 진입점이며, 세부 작업은 각각 독립 호출할 수 있습니다.

| 스킬 | 맡는 작업 |
|---|---|
| [`$ap`](skills/ap/SKILL.md) | 기존 계획을 진행 창에 표시하고 일반 요청을 연결·테마·복구로 안내 |
| [`$ap-connect`](skills/ap-connect/SKILL.md) | 에이전트 연결, 자동 창 열기, 위치 변경, 정확한 세션 진단 |
| [`$ap-theme`](skills/ap-theme/SKILL.md) | 색상 프리셋, 밝기, 상태별 색, YAML 테마 |
| [`$ap-recover`](skills/ap-recover/SKILL.md) | 저장된 진행 상황 복귀, 근거, 백업·복원·이동 |

Codex에게 다음처럼 요청하면 됩니다.

```text
https://github.com/justn-hyeok/agent-progress/tree/main/skills 의 ap, ap-connect, ap-theme, ap-recover 스킬을 설치해줘. 기존 같은 이름의 스킬이 있으면 먼저 내용을 비교해줘.
```

또는 저장소나 릴리스 압축 파일의 `skills/`에서 각 스킬 폴더 전체를 Codex의 skills 디렉터리(기본 `~/.codex/skills/`) 아래 같은 이름으로 설치합니다. Homebrew는 스킬을 `share/agent-progress/skills/`에 보관하며 자동으로 사용자 스킬 설정을 바꾸지 않습니다. 기존 `$ap`만 설치한 환경도 그대로 사용할 수 있습니다. 설치 후 다음 턴부터 새 스킬을 호출할 수 있습니다.

```text
$ap 이 프로젝트의 기존 계획을 진행 창에 연결해줘.
$ap-connect 이 프로젝트 계획을 연결하고 진행 창을 위쪽에 띄워줘.
$ap-theme 배경을 좀 더 밝게 하고 막힘 상태 색을 바꿔줘.
$ap-recover 저장된 진행 상황과 근거를 확인하고 이어서 할 일을 알려줘.
```

스킬은 기존 목표와 계획을 유지합니다. 별도 체크리스트 등록이나 진행률을 맞추기 위한 완료 처리를 요구하지 않습니다.

## 화면과 진행 기준

작은 창은 목표·현재 작업·다음 작업·완료율을 보여줍니다. 창을 키우면 작업 목록, 완료 조건, 검증 근거와 이력이 나타납니다. `q` 또는 `Ctrl-C`로 진행 창을 종료합니다.

| 기능 | 동작 |
| --- | --- |
| 자동 관찰 | 기존 계획 변경과 작업 상태를 읽어 반영 |
| 창 관리 | 자동 생성, 중복 창 재사용, 세션 재연결 |
| 위·아래 배치 | 관리된 수직 형제 창의 위치 변경과 기존 크기·포커스 유지 |
| 테마 | 색상 프리셋, 배경 밝기, 상태별 색, YAML 조합과 실시간 반영 |
| 계획·근거 | 고정 작업 ID, 선행 작업, 완료 조건, 코드 변경에 따른 재검증 표시 |
| 복귀·보존 | 최근 작업 요약, 다중 목표 조회, 백업·복원·내보내기·가져오기 |
| 연동 | 선택적 파일 모드, 연결 진단, 로컬 stdio MCP |

완료율은 **체크리스트의 완료 비율**입니다. 남은 시간이나 검증된 제품 완성도를 뜻하지 않습니다. 에이전트의 완료 보고, 실제 프로세스 활동, 자동 검사, 사람 확인은 구분합니다. 계획이 없으면 완료율은 미정이며, 목록에서 빠진 미완료 항목을 임의로 완료·취소하지 않습니다.

`ap.project.json`이 있는 프로젝트는 그 제품 계획을 전체 기준으로 삼습니다. 세부 작업의 완료가 상위 수용 항목 전체를 완료시키지는 않습니다. [제품 계획 연결 계약](docs/core-connection.md)을 참고하세요.

## 크기·테마·위치

```sh
ap config show
ap config set --pane-size 25       # 높이 비율 25%; 허용 범위 10~50
ap config presets
ap config set --preset forest --brightness 1.2
ap config set --accent '#88AAFF'

# 위쪽 배치를 기본값으로 저장하고, 이 pane의 관리 창에도 바로 적용
ap config set --position above
```

Herdr 또는 `ap launch`가 만든 tmux의 **현재 소스 창**에서 크기를 바꾸면,
소유가 확인된 진행 창에 바로 반영됩니다. 다른 터미널에서 바꾸면 다음 창 열기부터
적용됩니다. 작은 창에서 Herdr나 tmux가 더 큰 최소 pane을 요구하면 그 한계를 따릅니다.

기본 프리셋은 `signal`, `forest`, `ocean`, `amber`입니다. 밝기는 0.25~2.0이며 배경에만 적용됩니다. 프리셋을 바꾸면 기존 개별 색 덮어쓰기는 해제됩니다.

색을 직접 조합하려면 현재 설정을 YAML로 옮깁니다.

```sh
ap config init-yaml
```

생성된 `.agent-progress/ui.yaml`에서 프리셋 상속, 색상별 덮어쓰기, 작업 중·대기·막힘·일시 중지·오류·완료·빈 상태의 색을 설정할 수 있습니다. [YAML 예제](docs/theme-example.yaml)와 [테마 스킬](skills/ap-theme/SKILL.md)을 참고하세요. YAML은 기존 `ui.json`보다 우선하며, 잘못된 편집 중에는 실행 중 창의 마지막 정상 색을 유지합니다.

## 복귀와 진단

제품 계획이 연결된 프로젝트에서:

```sh
ap product summary
ap product resume
```

이미 확인된 원본 pane의 연결 상태는 `ap doctor --pane SOURCE`로 진단합니다. 복구 명령과 충돌·백업 처리 방법은 [운영 문서](docs/operations.md)에 있습니다. 최신 로그나 현재 포커스만으로 다른 세션을 임의 선택하지 않습니다.

`ap doctor --agent codex`는 설치된 CLI, 현재 프로젝트 연결·제품 선언,
셸 설정과 터미널 백엔드를 읽기 전용으로 점검합니다. `healthy`는 진단 결과이며
기본 명령의 종료 코드는 계속 0입니다. 자동 검사에서 셸 설정이 없는 상태는
제품 결함으로 계산하지 않습니다. 자동화에서 선택한 검사가 문제면 실패하도록
하려면 `ap doctor --agent codex --strict`를 사용합니다. 셸만 정확히 확인할 때는
`ap doctor --shell zsh --rc PATH --strict`를 사용합니다. 모델 요청·설정 복구는 하지 않습니다.

Herdr 밖에서도 파일 모드, 명시한 Codex 세션 기록 읽기, 테마 설정과 MCP를 사용할 수 있습니다. 자세한 명령은 `ap --help`와 각 하위 명령의 `--help`에서 확인하세요.

## 지원과 검증

릴리즈 검증 환경은 macOS 26.6.2 arm64, Herdr 0.9.0, Codex CLI 0.157.1와 공유 서버 0.158.0, Claude Code 2.1.274, OpenCode 1.18.30입니다. 다른 OS·아키텍처와 에이전트 버전의 호환성은 별도로 확인해야 합니다. Codex 자동 연결은 `ap launch --agent codex`와 그 경로로 전달하는 셸 연결 함수를 사용합니다.

실행기는 버전이 바뀌면 연결 인터페이스를 자동 재검사하고 문제를 경고합니다.
Codex는 실제 생성한 RPC 스키마도 검사합니다. 결과는 프로젝트의
`.agent-progress/compatibility/`에 저장하며, 실패해도 원래 에이전트 실행은 유지합니다.

```sh
ap compatibility                         # 로컬 CLI/RPC 검사, 모델 호출 없음
ap compatibility --agent codex --live    # 인증된 짧은 계획 응답 검사
opencode models opencode-go              # 현재 사용 가능한 모델 확인
ap compatibility --agent opencode --live --model opencode-go/gpt-6-luna
```

`--live`는 임시 디렉터리에서 짧은 모델 요청을 하므로 기존 인증과 사용량이 필요합니다.
계획 응답이 HUD에서 1/2로 해석되는지 검사하며, 모든 native task 도구나 창 배치의
호환성을 대신 인증하지 않습니다. 실패는 종료 코드와 JSON으로 반환합니다.

Apple Developer 서명·공증은 없습니다. 로컬 세션 원문, 인증 파일과 사용자 화면 캡처는 공개 저장소에 포함하지 않습니다.

v1.1.0은 Rust 테스트 83개, fmt·Clippy, 실제 터미널 입출력 검사와 설치 패키지 검사를 통과했습니다. Homebrew 설치·실행 검사도 별도 CI에서 실행합니다. [GitHub Actions](https://github.com/justn-hyeok/agent-progress/actions)에서 커밋별 결과를 확인할 수 있습니다.

2026-09-29에는 설치된 세 하네스에서 native 계획 갱신과 세션 복귀를 다시 검증했습니다. `1.1.1`은 Codex 실행기에서 공유 서버의 창별 세션을 식별하는 경로를 추가합니다. [하네스별 검사 결과와 연결 방식](docs/verification/native-harnesses.md)을 참고하세요.

## 소스 빌드

```sh
cargo build --locked --release
./target/release/ap --version

cargo fmt --check
HERDR_ENV=0 cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

상세 설계와 저장 형식은 [제품 범위](docs/product-and-design.md), [저장 형식](docs/storage-v1.md), [세션 입력](docs/session-follow.md), [운영·복구](docs/operations.md)를 참고하세요.
