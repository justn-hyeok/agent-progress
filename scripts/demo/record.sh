#!/bin/bash
# Record the README demo of the real `ap view` TUI with VHS into docs/media/.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
command -v vhs >/dev/null || { echo "install VHS: brew install vhs" >&2; exit 1; }
cargo build --release --locked --manifest-path "$root/Cargo.toml" >/dev/null
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/project" && git -C "$work/project" init -q
cp "$root/scripts/demo/drive.sh" "$work/drive.sh"
cat > "$work/demo.tape" <<TAPE
Output progress.mp4
Output progress.gif
Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 24
Set LetterSpacing 0
Set LineHeight 1.15
Set Width 1500
Set Height 214
Set Padding 0
Set Margin 0
Set WindowBar ""
Set TypingSpeed 0
Set Theme { "background": "#0C1316", "foreground": "#F2F5EE" }
Hide
Type "AP_BIN='$root/target/release/ap' DEMO_PROJECT='$work/project' bash ./drive.sh"
Enter
Sleep 700ms
Show
Sleep 30s
TAPE
# VHS occasionally fails to start its browser; one retry is enough in practice.
record() { (cd "$work" && env -u HERDR_ENV -u HERDR_PANE_ID -u TMUX -u TMUX_PANE vhs demo.tape); }
record || { echo "retrying VHS once" >&2; record; }
mkdir -p "$root/docs/media"
cp "$work/progress.mp4" "$work/progress.gif" "$root/docs/media/"
echo "docs/media/progress.mp4, docs/media/progress.gif"
