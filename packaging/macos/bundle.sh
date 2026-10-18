#!/usr/bin/env bash
# Builds "Honey Interactive Learn.app" from the hil-desktop binary.
#
# Usage: packaging/macos/bundle.sh [--install] [--zip] [--skip-build] [--target <triple>]
#   --install     copy the app to /Applications (or ~/Applications if not writable)
#   --zip         also write target/Honey-Interactive-Learn-<version>-<arch>.zip
#   --skip-build  reuse an existing release binary
#   --target      cargo target triple (default: host)
set -euo pipefail

APP_NAME="Honey Interactive Learn"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HERE="$ROOT/packaging/macos"
install=0 zip=0 build=1 target=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install) install=1 ;;
    --zip) zip=1 ;;
    --skip-build) build=0 ;;
    --target) target="$2"; shift ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
if [[ -n "$target" ]]; then
  bin="$ROOT/target/$target/release/hil-desktop"
  arch="${target%%-*}"
  target_args=(--target "$target")
else
  bin="$ROOT/target/release/hil-desktop"
  arch="$(uname -m)"
  target_args=()
fi

if [[ $build -eq 1 ]]; then
  (cd "$ROOT" && cargo build --release -p hil-desktop "${target_args[@]+"${target_args[@]}"}")
fi
[[ -x "$bin" ]] || { echo "missing $bin (build first)" >&2; exit 1; }

app="$ROOT/target/$APP_NAME.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/hil-desktop"
cp "$HERE/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$version/g" "$HERE/Info.plist" > "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"

# Ad-hoc signature so Apple Silicon runs it; not notarized.
codesign --force --deep --sign - "$app" >/dev/null
echo "built: $app ($version, $arch)"

if [[ $zip -eq 1 ]]; then
  out="$ROOT/target/Honey-Interactive-Learn-$version-$arch.zip"
  rm -f "$out"
  (cd "$ROOT/target" && ditto -c -k --keepParent "$APP_NAME.app" "$out")
  echo "zipped: $out"
fi

if [[ $install -eq 1 ]]; then
  dest="/Applications"
  [[ -w "$dest" ]] || { dest="$HOME/Applications"; mkdir -p "$dest"; }
  rm -rf "$dest/$APP_NAME.app"
  ditto "$app" "$dest/$APP_NAME.app"
  echo "installed: $dest/$APP_NAME.app"
fi
