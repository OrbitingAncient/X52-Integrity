#!/usr/bin/env bash
# Build an x86_64 AppImage from target/release. Requires appimagetool on PATH
# (https://github.com/AppImage/appimagetool/releases).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VER="${VERSION:-0.2.0}"
REL="$ROOT/target/release"
OUT="$ROOT/dist"
APPDIR="$OUT/X52Integrity.AppDir"

if [[ ! -x "$REL/x52-integrity" ]]; then
  echo "build first: cargo build --release" >&2
  exit 1
fi

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/metainfo" \
         "$APPDIR/usr/lib/udev/rules.d" "$OUT"

cp "$REL/x52-integrity" "$APPDIR/usr/bin/"
[[ -x "$REL/x52-monitor" ]] && cp "$REL/x52-monitor" "$APPDIR/usr/bin/"
cp "$ROOT/packaging/linux/x52-integrity.desktop" "$APPDIR/"
cp "$ROOT/packaging/linux/x52-integrity.desktop" "$APPDIR/usr/share/applications/"
cp "$ROOT/packaging/linux/99-x52-integrity.rules" "$APPDIR/usr/lib/udev/rules.d/"
cp "$ROOT/packaging/linux/AppRun" "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun" "$APPDIR/usr/bin/x52-integrity"

# Tiny PNG-less icon fallback: AppImage accepts a .desktop Icon= name.
if command -v convert >/dev/null 2>&1; then
  convert -size 256x256 xc:#4a82d6 "$APPDIR/x52-integrity.png" || true
fi

if command -v appimagetool >/dev/null 2>&1; then
  ARCH=x86_64 appimagetool "$APPDIR" "$OUT/X52-Integrity-${VER}-x86_64.AppImage"
  echo "Wrote $OUT/X52-Integrity-${VER}-x86_64.AppImage"
else
  echo "appimagetool not found; AppDir staged at $APPDIR" >&2
  echo "Download appimagetool and rerun, or ship the .deb instead." >&2
  exit 0
fi
