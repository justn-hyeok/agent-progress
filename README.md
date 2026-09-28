# agent-progress

에이전트용 스킬은 [skills/ap/SKILL.md](skills/ap/SKILL.md)에 있다. Codex의
skills 디렉터리에 `skills/ap` 폴더를 복사하면 `$ap`로 호출할 수 있다.
연결·진행 보고·테마·복귀/복구 사용법을 포함하며 앱 설치와 스킬 설치는 별개다.

macOS Apple Silicon에서 Homebrew로 설치:

```sh
brew tap justn-hyeok/agent-progress https://github.com/justn-hyeok/agent-progress
brew install justn-hyeok/agent-progress/agent-progress
ap --version
```

업데이트는 `brew update` 후 `brew upgrade agent-progress`, 제거는
`brew uninstall agent-progress`다. 프로젝트 데이터와 에이전트 설정은 보존된다.

1.1.0: 색상 프리셋, 배경 밝기, 상태별 배경과 YAML 조합을 지원한다.
Herdr 자동 호출과 위/아래 배치, Codex/Claude Code/OpenCode 연결, 공통 모델과
코드 지문 근거, 계획 변경/복구/다중 목표, 로컬 MCP를 제공한다.

[다운로드](https://github.com/justn-hyeok/agent-progress/releases/latest)에서
macOS arm64 패키지와 SHA-256을 받는다. [설치](docs/distribution.md),
[운영·복구와 테마](docs/operations.md)를 참고한다. Apple Developer 서명/공증은 없다.
로컬 세션 원문과 사용자 화면 캡처는 공개 저장소에 포함하지 않는다.

Herdr에서 에이전트 아래의 **별도 터미널**에 띄우는 진행 상황 창.
기존 goal·plan을 체크리스트로 가져와 완료/전체 체크율을 자동으로 표시한다.

현재 지원 실증 대상: macOS 26.6.2 arm64, Herdr 0.9.0, Codex 0.157.1,
Claude Code 2.1.274, OpenCode 1.18.30. 지원 버전의 입력/재시작 증거를 별도 기록한다.

- [제품 범위와 설계](docs/product-and-design.md)
- [제품 전체 완료 계획](docs/product-completion-plan.md)
- [현재 상황과 이어 할 일](docs/handoff.md)
- [현재 구현 계획과 남은 제품 범위](docs/current-plan.md)
- [저장 형식과 신뢰 경계](docs/storage-v1.md)
- [세션 연결 방식과 지원 범위](docs/session-follow.md)
- [제품 목표와 세션 계획의 연결 계약](docs/core-connection.md)

## 실행

Rust/Cargo로 한 번 빌드하고, **Codex pane 바로 아래에 split한 터미널**에서
실행한다. init/add나 작업 UUID 입력은 필요 없다.

```sh
cargo build --locked
./target/debug/ap
```

위 pane의 정확한 Codex 프로세스가 열고 있는 세션을 연결한다. 현재 포커스,
같은 폴더의 최신 파일, 임의의 다른 세션으로 연결하지 않는다. 다른 배치라면
`ap follow --pane <에이전트-pane-ID>`로 한 번 연결 대상을 지정한다.

에이전트가 자기 pane에서 `ap open`을 호출하면 아래쪽에 새 창을 열고 정확한
세션을 넘긴다. 이미 진행 상황 창이 있다면 새로 열 필요가 없다.

계획에서 항목이 빠져도 기존 완료 기록과 미완료 항목은 보존한다. 빠진 미완료는
`!`로 표시하고 체크율 분모에 남긴다. 목표가 바뀌면 이전 체크리스트를 이력에
보관한다. 계획이 없으면 체크율은 **미정**이다.

진행 상황 창은 조작 없이 읽는다. 창 전체의 채움은 제품 체크리스트 완료 비율이며,
텍스트에는 제품 목표·현재 계획·계획상 작업·실제 제품 계획 파일을 표시한다.
에이전트가 명시한 진행 계획의 `목표:`가 있으면 그 문구를 현재 계획 이름으로 쓴다.
5줄에서는 제품 목표 선언 파일도 보인다. 3줄은 목표·현재 상태·제품 계획 파일,
1줄은 비율·현재 상태를 우선한다. 창을 키우면 마지막 변경 시각과 체크리스트가
자동으로 보인다. 계획 파일이 없는 Codex native Plan은 세션 출처로 표시한다.
종료할 때는 `q` 또는 `Ctrl-C`를 누른다.

완료 체크는 **에이전트 보고**다. 테스트 실행 결과나 사람 확인을 증명하지 않는다.
Herdr의 작업 중/입력 대기 표시는 체크율과 별도의 활동 관측이다.

## 어떤 계획을 읽나

`ap.project.json`이 있으면 기존 제품 계획을 전체 기준으로 삼는다. 이 repo는
제품 계획의 AP-01~AP-24에 연결돼 있다. 임시 작업 종료나 세션 교체로 제품 목표와
완료 수를 초기화하지 않는다. 전체 항목은 `[AP-05]`, 부분 작업은 `[AP-05/native]`
처럼 연결한다. 부분 작업 완료만으로 전체 항목을 완료 처리하지 않는다.

Codex native Plan 모드의 `Plan` 이벤트와 해당 thread의 goal을 읽는다.
일반 실행의 명시적 진행 보고도 같은 계획의 상태를 이어 갱신한다.
`update_plan`/`plan_update` 입력도 지원하지만, 현재 설치본의 일반 실행에서는
그 tool이 노출되지 않으므로 native Plan 모드와 구별한다. goal DB는 필수가 아니다.

native plan 도구가 없는 환경에서는 에이전트가 대화에 작성하는 아래 형식을
읽는다. **사용자가 항목을 등록하는 절차가 아니라 에이전트의 개발 계획 형식**이다.
현재 세션은 이 경로로 실제 연결했다.

```markdown
### 진행 계획
목표: 로그인 기능 완성
- [x] 구조 확인
- [>] API 구현
- [ ] 테스트
```

일반 대화 목록이나 사용자가 붙인 예시를 무작정 작업으로 추정하지 않는다.
native Plan 뒤의 명시적 실행 보고도 반영하며 출처를 구별한다. 세션 목표 변경은
제품 목표/전체 항목을 교체하지 않는다. 코드 블록·인용문 속 예시는 제외한다. 원본 세션과 하네스
설정은 수정하지 않는다. 세부 지원 형식은 [연결 문서](docs/session-follow.md)를 참고한다.

## 파일 모드와 진단

기존 `ap/demo.md`와 `--file … init/add/status/evidence/watch` 명령은 보존했다.
이 파일 모드는 자동 연결의 필수 단계가 아니다. 세부 명령은 `ap --help`로
확인할 수 있다. 아래 명령은 UI 없이 한 번 관찰한 결과를 출력한다. 독립 세션은
원본을 읽기만 하고, 제품 선언에 연결된 경우 제품의 영속 진행 기록을 갱신한다.

```sh
./target/debug/ap follow --pane <에이전트-pane-ID> --once
./target/debug/ap follow --session <Codex-session-ID> --once
```

세션 checkpoint는 `.agent-progress/<session-ID>.json`, 제품 기록은
`.agent-progress/project-<제품-ID>.json`에 저장한다. 새 세션에서도 같은 제품
선언에 연결되면 전체 진행을 이어간다. 임의의 세션 교체나
손상은 자동으로 다른 데이터로 대체하지 않고 오류/오래된 상태로 표시한다.

## 검증

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
python3 scripts/pty_smoke.py
python3 scripts/pty_follow.py
python3 scripts/pty_review.py
python3 scripts/pty_product.py
```

`python3 scripts/native_contract.py`는 실제 Codex 모델 호출이 있는 native 연동
검사다. 사용자 세션 대신 별도 테스트 세션과 선언을 사용한다.

Rust 테스트는 도메인/저장, 실제 CLI, 세션 입력·잘못된 연결·계획 동기화,
한국어 TUI를 검사한다. PTY 테스트는 fixture 세션의 실시간 변경, 오류 복구,
재실행, 80×14→40×12와 q/Ctrl-C 복원을 검사한다. 실제 Herdr 증거와 fixture
검증은 [인계 문서](docs/handoff.md)에 구분한다.
