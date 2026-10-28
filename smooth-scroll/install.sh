#!/bin/sh
# Install the freshly built app into its own versioned bundle and point
# builds/current at it. Never update a bundle that a running Ghostty uses:
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
dest="builds/Ghostty-$rev.app"

mkdir -p builds
rm -rf "$dest.tmp"
cp -R "$src" "$dest.tmp"
codesign --force --deep --sign - "$dest.tmp" >/dev/null 2>&1
codesign --verify "$dest.tmp"
if [ -e "$dest" ]; then mv "$dest" "$HOME/.Trash/$(basename "$dest").$(date +%s)"; fi
mv "$dest.tmp" "$dest"
ln -sfn "$(basename "$dest")" builds/current
echo "installed $dest"
