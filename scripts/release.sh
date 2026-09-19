#!/usr/bin/env bash
# Builds a signed, notarized and stapled Parla DMG for distribution.
#
# One-time setup, which keeps the app-specific password in the Keychain:
#   xcrun notarytool store-credentials parla-notary --apple-id <apple-id> --team-id 5PNLAR99PK
set -euo pipefail
cd "$(dirname "$0")/.."

IDENTITY="${APPLE_SIGNING_IDENTITY:-Developer ID Application: Bryan Bernardo Parreira (5PNLAR99PK)}"
PROFILE="${NOTARY_PROFILE:-parla-notary}"
VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
BUNDLE="src-tauri/target/release/bundle"
APP="$BUNDLE/macos/Parla.app"
DMG="$BUNDLE/Parla_${VERSION}_aarch64.dmg"

APPLE_SIGNING_IDENTITY="$IDENTITY" pnpm tauri build --bundles app
codesign --verify --deep --strict "$APP"

# Staple the app itself too, so it still opens offline once copied out of the DMG.
ditto -c -k --keepParent "$APP" "$BUNDLE/Parla.zip"
xcrun notarytool submit "$BUNDLE/Parla.zip" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$APP"
rm "$BUNDLE/Parla.zip"

STAGING="$(mktemp -d)"
RW_DMG="$STAGING.dmg"
MOUNT=""
trap '[ -n "$MOUNT" ] && hdiutil detach "$MOUNT" -quiet 2>/dev/null; rm -rf "$STAGING" "$RW_DMG"' EXIT
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
# Finder only picks the 2x background on Retina displays when both are in one TIFF.
# Regenerate the PNGs with `swift design/dmg-background.swift design`.
mkdir "$STAGING/.background"
tiffutil -cathidpicheck design/dmg-background.png design/dmg-background@2x.png \
  -out "$STAGING/.background/background.tiff"

# The window layout lives in the volume's .DS_Store, which only Finder writes, so the
# image is built writable, styled through Finder, then compressed.
hdiutil create -volname Parla -srcfolder "$STAGING" -fs HFS+ -format UDRW -ov "$RW_DMG"
# Another Parla DMG may already be mounted, in which case this one lands at
# "/Volumes/Parla 1", so Finder is pointed at the mount this attach actually made.
MOUNT="$(hdiutil attach "$RW_DMG" -readwrite -noverify -noautoopen | grep -o '/Volumes/.*$')"
# Icon positions must match the layout in design/dmg-background.swift.
osascript - "$(basename "$MOUNT")" <<'APPLESCRIPT'
on run argv
tell application "Finder"
  tell disk (item 1 of argv)
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set bounds of container window to {200, 120, 840, 548}
    set viewOptions to the icon view options of container window
    set arrangement of viewOptions to not arranged
    set icon size of viewOptions to 112
    set text size of viewOptions to 13
    set background picture of viewOptions to file ".background:background.tiff"
    set position of item "Parla.app" of container window to {170, 205}
    set position of item "Applications" of container window to {470, 205}
    close
    open
    update without registering applications
    delay 1
    close
  end tell
end tell
end run
APPLESCRIPT
rm -rf "$MOUNT/.fseventsd"
sync
hdiutil detach "$MOUNT" -quiet
MOUNT=""
hdiutil convert "$RW_DMG" -format UDZO -imagekey zlib-level=9 -ov -o "$DMG"

codesign --force --sign "$IDENTITY" --timestamp "$DMG"
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$DMG"
spctl -a -t open --context context:primary-signature -v "$DMG"

echo "$DMG"
