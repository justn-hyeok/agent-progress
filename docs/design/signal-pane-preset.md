# Signal pane — 수정 시안 / 승인 대기

2026-09-28. Quiet pane은 사용자 거절: 더 트렌딩한 방향 요청.
범위: AP-16/ui. 정보·상태 의미와 수동 조작 없는 기존 동작은 유지한다.
Codex-only 프로젝트 정책에 따라 Codex가 직접 시각 방향을 구성했다.

- 사용자 추가 피드백: 픽셀 숫자가 별로임. 대형 block glyph를 제거했다.
- 시그니처: 라임 브랜드·체크율을 첫 줄에 묶고, 아래 4줄에 계획·상태·복귀·출처.
- 체크율은 일반 terminal 글자 크기의 굵은 `16%`, 대상 수는 옆의 `4 / 24`다.
- 좁은 창/5줄 미만은 핵심 정보와 수치를 우선하는 단일 열로 전환한다.
  큰 창의 기존 조건·근거·이력 자동 노출은 유지한다.
- 색: track #0F1214, 전체 면적 진행 fill #1F271B, accent #C7F96C,
  primary #F2F5EE, secondary #9BA49B, metadata #89988C, paused #F5C26F.
  출처 색은 시안보다 조금 밝게 보정해 진행 배경에서도 대비를 확보했다.
- 진행 배경은 계속 전체 면적의 완료/대상 비율이다. 숫자는 체크율이고
  제품 완성도·남은 시간·실행 상태를 뜻하지 않는다.
- 현재 계획이 본문, 상태는 짧고 굵은 별도 줄, 출처는 낮은 강조다.
  프로젝트 이름이 바뀌어도 고정 브랜드 라벨과 실제 목표를 구별한다.
- 미리보기의 타이포는 렌더용이며 실제 구현은 terminal bold/normal로 제한한다.
- 모션: N/A. 그래디언트·카드·필수 단축키·새 의존성을 추가하지 않는다.

리스크: 첫 줄에 목표·수치가 모인다. 좁은 창에서 수치를 보존하고 목표를
줄이며, 긴 한국어·paused/blocked/error와 미정 체크율을 검증한다.

목표: [AP-16/ui] 진행 창의 시각적 구성을 개선한다.
- [x] 사용자 거절을 반영해 terminal-cell 기반 단일 수정 시안을 준비한다.
- [x] Signal 프리셋 승인 후 대표 TUI에 적용한다.
- [x] 크기별 렌더와 실제 Herdr 창을 검증한다.

승인 상태: approved. 사용자 “오 더 나은 듯”은 수정 시안 선택으로 기록했다.
Receipt: {"gate":"design-preset","selectedPreset":"signal-refined","approvedByUser":true,"scope":["AP-16/ui: default TUI palette, hierarchy, grouped metric"]}
적용과 실제 창 검증 완료. `docs/evidence/signal-ui-2026-09-28.json` 참조.
