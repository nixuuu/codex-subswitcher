#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile=${1:-release}
case "$profile" in
  release) cargo build --release --locked ;;
  debug) cargo build --locked ;;
  *) echo 'Usage: scripts/bundle.sh [release|debug]' >&2; exit 2 ;;
esac
app='dist/Codex Sub Switcher.app'
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$profile/codex-sub-switcher" "$app/Contents/MacOS/codex-sub-switcher"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>tech.itsol.codex-sub-switcher</string>
<key>CFBundleName</key><string>Codex Sub Switcher</string>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleLocalizations</key><array><string>en</string></array>
<key>CFBundleExecutable</key><string>codex-sub-switcher</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$app"
printf '%s\n' "$app"
