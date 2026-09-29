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

```sh
# 업데이트
brew update
brew upgrade agent-progress

# 제거 — 프로젝트 데이터는 보존
brew uninstall agent-progress
```

## 에이전트 연결

Herdr 안에서 작업할 프로젝트로 이동한 뒤, 사용할 에이전트를 연결합니다. Codex의 경우:

```sh
ap connect preview --agent codex
ap connect apply --agent codex
ap launch --agent codex
```

`preview`는 적용 내용을 보여주고, `apply`는 프로젝트의 연결 설정을 백업한 뒤 추가합니다. `launch`는 설치된 Codex를 실행하고, 그 창의 실제 세션 시작·복귀 응답을 읽어 진행 창을 연결합니다. 기존 공유 서버를 끄거나 세션 ID를 입력할 필요가 없습니다. 프로젝트 신뢰나 훅 승인이 요구되면 해당 에이전트에서 확인하세요.

연결된 훅이 원본 세션을 확인하면 진행 창을 자동으로 엽니다. 에이전트는 기존 계획 도구를 그대로 사용합니다. 원본이 확정된 에이전트 창에서 `ap open`으로 직접 열 수도 있습니다.

| 에이전트 | 연결 명령 | 읽는 계획 |
| --- | --- | --- |
| Codex | `ap connect apply --agent codex` | Native Plan·명시적 진행 보고·선택적 goal |
| Claude Code | `ap connect apply --agent claude` | TodoWrite·TaskCreate/TaskUpdate·명시적 진행 보고 |
| OpenCode | `ap connect apply --agent opencode` | Native todos·명시적 진행 보고 |

Claude Code와 OpenCode도 설정 적용 후 훅·플러그인을 읽는 세션에서 작업합니다. 연결 설정은 프로젝트 안에서만 관리하며 에이전트 전역 설정은 바꾸지 않습니다. [연결 조건과 진단](docs/operations.md)을 참고하세요.

## `$ap` 스킬

에이전트에게 연결·테마·복구를 맡기려면 [ap 스킬](skills/ap/SKILL.md)을 설치하세요. **앱의 Homebrew 설치와 스킬 설치는 별개입니다.**

Codex에게 다음처럼 요청하면 됩니다.

```text
https://github.com/justn-hyeok/agent-progress/tree/main/skills/ap 스킬을 설치해줘.
```

또는 저장소의 `skills/ap` 폴더 전체를 Codex의 skills 디렉터리(기본 `~/.codex/skills/ap`)로 복사합니다. 기존 같은 이름의 스킬이 있다면 먼저 내용을 확인하세요. 설치 후 다음 턴부터 호출할 수 있습니다.

```text
$ap 이 프로젝트의 기존 계획을 진행 창에 연결해줘.
$ap 배경을 좀 더 밝게 하고 위쪽에 띄워줘.
$ap 연결이 끊긴 이유를 확인하고 복구해줘.
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

## 테마와 위치

```sh
ap config presets
ap config set --preset forest --brightness 1.2
ap config set --accent '#88AAFF'

# 위쪽 배치를 기본값으로 저장하고 적용
ap config set --position above
ap open
```

기본 프리셋은 `signal`, `forest`, `ocean`, `amber`입니다. 밝기는 0.25~2.0이며 배경에만 적용됩니다. 프리셋을 바꾸면 기존 개별 색 덮어쓰기는 해제됩니다.

색을 직접 조합하려면 현재 설정을 YAML로 옮깁니다.

```sh
ap config init-yaml
```

생성된 `.agent-progress/ui.yaml`에서 프리셋 상속, 색상별 덮어쓰기, 작업 중·대기·막힘·일시 중지·오류·완료·빈 상태의 색을 설정할 수 있습니다. [YAML 예제](docs/theme-example.yaml)와 [설정 설명](skills/ap/references/themes.md)을 참고하세요. YAML은 기존 `ui.json`보다 우선하며, 잘못된 편집 중에는 실행 중 창의 마지막 정상 색을 유지합니다.

## 복귀와 진단

제품 계획이 연결된 프로젝트에서:

```sh
ap product summary
ap product resume
```

이미 확인된 원본 pane의 연결 상태는 `ap doctor --pane SOURCE`로 진단합니다. 복구 명령과 충돌·백업 처리 방법은 [운영 문서](docs/operations.md)에 있습니다. 최신 로그나 현재 포커스만으로 다른 세션을 임의 선택하지 않습니다.

Herdr 밖에서도 파일 모드, 명시한 Codex 세션 기록 읽기, 테마 설정과 MCP를 사용할 수 있습니다. 자세한 명령은 `ap --help`와 각 하위 명령의 `--help`에서 확인하세요.

## 지원과 검증

릴리즈 검증 환경은 macOS 26.6.2 arm64, Herdr 0.9.0, Codex CLI 0.157.1와 공유 서버 0.158.0, Claude Code 2.1.274, OpenCode 1.18.30입니다. 다른 OS·아키텍처와 에이전트 버전의 호환성은 별도로 확인해야 합니다. Codex 공유 서버의 자동 창 연결은 위의 `ap launch --agent codex` 경로에서 검증합니다.

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
