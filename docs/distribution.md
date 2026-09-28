# macOS 배포

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
tar.gz와 SHA-256 파일로 제공한다. 서명/공증에는 Apple Developer 계정이 필요하다.
설치 자동 검사는 임시 prefix에서 수행한다.
