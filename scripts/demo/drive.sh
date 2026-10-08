#!/bin/bash
# Run inside the VHS terminal: a 7-step plan completing every 3.5 s while `ap view` shows it.
set -u
unset HERDR_ENV HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID TMUX TMUX_PANE CODEX_THREAD_ID AP_PLAN
export AP_AUTO_OPEN=0
AP=$AP_BIN
cd "$DEMO_PROJECT"
"$AP" --plan uidemo goal "UI 시연: 차가운 딥그린·화살표로 사라지는 빛" >/dev/null
"$AP" --plan uidemo add "요구사항 정리" "저장소 설계" "CLI 구현" "진행 창 그리기" "애니메이션" "테스트" "릴리즈" >/dev/null
"$AP" --plan uidemo start 1 >/dev/null
(
  sleep 2
  for i in 1 2 3 4 5 6 7; do
    "$AP" --plan uidemo done "$i" >/dev/null
    [ "$i" -lt 7 ] && "$AP" --plan uidemo start $((i + 1)) >/dev/null
    sleep 3.5
  done
) &
clear
exec "$AP" --plan uidemo view
