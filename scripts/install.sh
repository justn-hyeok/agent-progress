#!/bin/sh
# Local unsigned distribution. User data and agent configuration are never removed.
set -eu
package_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ap_prefix=${HOME}/.local
ap_action=install
while [ "$#" -gt 0 ]; do
  case "$1" in
    --prefix) ap_prefix=$2; shift 2 ;;
    --rollback) ap_action=rollback; shift ;;
    --uninstall) ap_action=uninstall; shift ;;
    *) echo "usage: install.sh [--prefix PATH] [--rollback|--uninstall]" >&2; exit 2 ;;
  esac
done
case "$ap_prefix" in /*) ;; *) echo "prefix must be absolute" >&2; exit 2 ;; esac
ap_base=$ap_prefix/lib/agent-progress
ap_link=$ap_prefix/bin/ap
[ ! -L "$ap_prefix" ] && [ ! -L "$ap_prefix/lib" ] && [ ! -L "$ap_prefix/bin" ] && [ ! -L "$ap_base" ] && [ ! -L "$ap_base/releases" ] || { echo "refusing symlink managed directory" >&2; exit 1; }
mkdir -p "$ap_base"
mkdir "$ap_base/.install-lock" 2>/dev/null || { echo "installation is busy" >&2; exit 1; }
ap_stage=
trap '[ -z "${ap_stage:-}" ] || { rm -f "$ap_stage/ap" "$ap_stage/VERSION" "$ap_stage/SHA256SUMS" "$ap_stage/THIRD_PARTY.md" "$ap_stage/OPERATIONS.md"; rmdir "$ap_stage" 2>/dev/null || :; }; rmdir "$ap_base/.install-lock" 2>/dev/null || :; rmdir "$ap_base" 2>/dev/null || :' EXIT
if [ -e "$ap_link" ] || [ -L "$ap_link" ]; then
  [ -L "$ap_link" ] || { echo "refusing unrelated bin/ap" >&2; exit 1; }
  ap_old=$(readlink "$ap_link")
  case "$ap_old" in "$ap_base"/releases/*/ap) ;; *) echo "refusing unrelated ap link" >&2; exit 1 ;; esac
else ap_old=; fi
if [ "$ap_action" = uninstall ]; then
  [ -f "$ap_base/OWNED" ] && [ "$(cat "$ap_base/OWNED")" = agent-progress-v1 ] || { echo "managed installation absent" >&2; exit 1; }
  [ -z "$ap_old" ] || rm -- "$ap_link"
  # Remove only the package's exact known files. Unknown/user files are retained.
  for ap_release in "$ap_base"/releases/*; do
    [ -d "$ap_release" ] && [ ! -L "$ap_release" ] || continue
    for ap_file in ap VERSION SHA256SUMS THIRD_PARTY.md OPERATIONS.md; do
      [ ! -f "$ap_release/$ap_file" ] || rm -- "$ap_release/$ap_file"
    done
    rmdir "$ap_release" 2>/dev/null || :
  done
  for ap_file in previous OWNED; do [ ! -f "$ap_base/$ap_file" ] || rm -- "$ap_base/$ap_file"; done
  rmdir "$ap_base/releases" "$ap_base" 2>/dev/null || :
  echo 'uninstalled; project data and agent settings retained'
  exit 0
fi
mkdir -p "$ap_prefix/bin" "$ap_base/releases"
[ ! -L "$ap_base" ] && [ ! -L "$ap_base/releases" ] && [ ! -L "$ap_prefix/bin" ] || { echo "refusing symlink managed directory" >&2; exit 1; }
if [ "$ap_action" = rollback ]; then
  [ -f "$ap_base/previous" ] || { echo "no previous release" >&2; exit 1; }
  ap_target=$(cat "$ap_base/previous")
  case "$ap_target" in "$ap_base"/releases/*/ap) ;; *) echo "invalid previous release" >&2; exit 1 ;; esac
  [ -x "$ap_target" ] || { echo "previous release unavailable" >&2; exit 1; }
else
  [ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] || { echo "this package supports macOS arm64" >&2; exit 1; }
  ap_version=$(cat "$package_dir/VERSION")
  case "$ap_version" in ''|.|..|*[!0-9A-Za-z._-]*) echo "invalid package version" >&2; exit 1 ;; esac
  (cd "$package_dir" && /usr/bin/shasum -a 256 -c SHA256SUMS)
  ap_release=$ap_base/releases/$ap_version
  ap_target=$ap_release/ap
  if [ -d "$ap_release" ]; then
    cmp "$package_dir/ap" "$ap_target" >/dev/null || { echo "same version has different bytes; use a new version" >&2; exit 1; }
  else
    ap_stage=$(mktemp -d "$ap_base/releases/.stage-XXXXXX")
    for ap_file in ap VERSION SHA256SUMS THIRD_PARTY.md OPERATIONS.md; do cp "$package_dir/$ap_file" "$ap_stage/$ap_file"; done
    chmod 755 "$ap_stage/ap"
    "$ap_stage/ap" --version
    mv "$ap_stage" "$ap_release"
    ap_stage=
  fi
fi
# Replace the link atomically in its own directory; never overwrite an unrelated file.
ap_tmp=$(mktemp -d "$ap_prefix/bin/.ap-link-XXXXXX")
ln -s "$ap_target" "$ap_tmp/ap"
mv -f "$ap_tmp/ap" "$ap_link"
rmdir "$ap_tmp"
[ -z "$ap_old" ] || printf '%s\n' "$ap_old" > "$ap_base/previous"
printf '%s\n' agent-progress-v1 > "$ap_base/OWNED"
echo "installed $ap_target; add $ap_prefix/bin to PATH if needed"
