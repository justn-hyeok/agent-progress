# macOS 배포

## Homebrew

기존 저장소의 `Formula/agent-progress.rb`를 사용자 지정 tap으로 제공한다.

```sh
brew tap justn-hyeok/agent-progress https://github.com/justn-hyeok/agent-progress
brew install justn-hyeok/agent-progress/agent-progress
```

Homebrew가 릴리즈 archive의 SHA-256을 검증하고 `ap`를 설치한다.
압축 파일의 `skills/`에는 `$ap`, `$ap-connect`, `$ap-theme`, `$ap-recover`
Codex 스킬이 들어 있다. Homebrew는 이를 formula의 `share/agent-progress/skills/`에
보관하며 사용자 Codex 스킬 디렉터리를 자동 변경하지 않는다. 필요한 폴더를
내용 확인 후 `~/.codex/skills/`에 따로 설치한다.
Rust, Node, Herdr는 설치 의존성이 아니다. macOS arm64 전용이다.
Herdr 밖의 자동 진행 창에는 선택적으로 tmux가 필요하다(`brew install tmux`).
셸 연결은 `ap shell install --shell zsh|bash|fish`로 설치하며 원본 실행 파일을 바꾸지 않는다.
`brew update && brew upgrade agent-progress`로 업데이트하고
`brew uninstall agent-progress`로 제거한다. `.agent-progress` 데이터는 보존한다.
자동 에이전트 연결은 설치 후 프로젝트에서 별도로 `ap connect apply`로 설정한다.
Homebrew 설치는 개발자 서명이나 공증을 추가하지 않는다.

새 버전 배포 시 formula의 version, url, sha256을 함께 갱신한다.

## 압축 패키지

현재 지원 실증 대상은 macOS 26.6.2 / arm64다. 다른 OS/아키텍처는 미검증이며
지원 완료로 표시하지 않는다. Apple Developer 배포 서명과 공증은 없다.
빌드 도구가 부여하는 로컬 ad-hoc 서명은 개발자 인증/공증의 증거가 아니다.
공개 다운로드·Gatekeeper 사용 흐름은 로컬 설치 검사와 구별한다.

개발자는 `python3 scripts/package.py --output dist`로 Cargo.lock에 고정된
release 바이너리와 tar.gz/checksum을 만든다. 실행 시 Rust·Node·서버는 불필요하다.
패키지의 설치 스크립트는 macOS 기본 sh/shasum을 사용한다.

패키지를 풀고 `sh install.sh --prefix /절대/설치/경로`를 실행한다. 기본 경로는
`~/.local`이며 bin/ap 링크를 만든다. PATH는 변경하지 않는다. 전역 agent 설정과
프로젝트의 `.agent-progress` 데이터도 수정하지 않는다.

업데이트는 새 패키지에서 같은 prefix로 설치한다. 이전 버전을 보존하며
`sh install.sh --prefix /같은/경로 --rollback`으로 되돌린다.
`--uninstall`은 관리된 실행 파일만 제거한다. 프로젝트 데이터는 보존한다.
동일 버전의 다른 바이너리, 무관한 bin/ap, symlink 관리 디렉터리는 거부한다.

배포물은 [GitHub Releases](https://github.com/justn-hyeok/agent-progress/releases)에
스킬을 포함한 tar.gz와 SHA-256 파일로 제공한다. 서명/공증에는 Apple Developer 계정이 필요하다.
설치 자동 검사는 임시 prefix에서 수행한다.
