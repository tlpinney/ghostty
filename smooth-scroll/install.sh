#!/bin/sh
# Install the freshly built app as "Ghostty Smooth" into its own versioned
# bundle and point builds/current at it. Never update a bundle that a running Ghostty uses:
# macOS kills the running process (Code Signature Invalid) when it pages
# in code that no longer matches the signature.
#
# Usage: smooth-scroll/install.sh   (after `zig build` and the xcodebuild step)
# Launch: builds/current/Contents/MacOS/ghostty
set -eu
cd "$(dirname "$0")/.."

src=macos/build/ReleaseLocal/Ghostty.app
rev=$(git rev-parse --short HEAD)
if ! git diff --quiet HEAD -- src; then rev="$rev-dirty-$(date +%H%M%S)"; fi
dest="builds/Ghostty-Smooth-$rev.app"
tmp="$dest.tmp"

# Own name, bundle id, and icon so the fork is distinct from the official
# Ghostty in the Dock, app switcher, and macOS permissions.
name="Ghostty Smooth"
bundle_id="com.tlpinney.ghostty-smooth"

trash() { if [ -e "$1" ]; then mv "$1" "$HOME/.Trash/$(basename "$1").$(date +%s)"; fi; }

mkdir -p builds
trash "$tmp"
cp -R "$src" "$tmp"
plist="$tmp/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $bundle_id" "$plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName $name" "$plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName $name" "$plist"
# The asset catalog icon would win over CFBundleIconFile; use the file.
/usr/libexec/PlistBuddy -c "Delete :CFBundleIconName" "$plist" 2>/dev/null || true
smooth-scroll/make-icon.py "$src/Contents/Resources/Ghostty.icns" "$tmp/Contents/Resources/Ghostty.icns"
codesign --force --deep --sign - "$tmp" >/dev/null 2>&1
codesign --verify "$tmp"
trash "$dest"
mv "$tmp" "$dest"
ln -sfn "$(basename "$dest")" builds/current
# Let the Dock and Spotlight pick up the new name and icon.
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$dest" || true
echo "installed $dest"
