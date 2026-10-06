# macOS 배포

## Homebrew

```sh
brew tap justn-hyeok/agent-progress https://github.com/justn-hyeok/agent-progress
brew install justn-hyeok/agent-progress/agent-progress
```

Homebrew가 릴리즈 archive의 SHA-256을 검증하고 `ap`를 설치합니다. 압축 파일의
`skills/ap`는 formula의 `share/agent-progress/skills/`에 보관되며, 사용자 에이전트
스킬 디렉터리는 자동으로 바꾸지 않습니다. 필요하면 내용을 확인한 뒤 직접 복사합니다.
Rust, Node, Herdr는 설치 의존성이 아닙니다. macOS arm64 전용입니다.

`brew upgrade agent-progress`로 업데이트하고 `brew uninstall agent-progress`로 제거합니다.
프로젝트의 `.agent-progress` 데이터는 보존합니다. 개발자 서명이나 공증은 없습니다.

## 압축 패키지

`python3 scripts/package.py --output dist`로 Cargo.lock에 고정된 release 바이너리와
tar.gz/checksum을 만듭니다. `python3 scripts/package_smoke.py <archive> [--previous <이전 archive>]`가 임시 prefix에서
업그레이드·롤백·재업그레이드·실행·제거를 확인합니다.

패키지를 풀고 `sh install.sh --prefix /절대/경로`로 설치합니다(기본 `~/.local`).
`--rollback`은 이전 버전으로, `--uninstall`은 관리된 실행 파일만 제거합니다.
