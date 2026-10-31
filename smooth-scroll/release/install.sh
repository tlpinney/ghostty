#!/bin/sh
# Installs a signed Ghostty Smooth release (macOS, Apple Silicon):
#   sh install.sh                      # latest release
#   sh install.sh smooth-v2026.10.31   # a specific release
# It installs only after verifying that
#   1. SHA256SUMS carries a valid SSH signature from the key below (made offline
#      by the maintainer, not by CI), and
#   2. the downloaded app archive matches its line in SHA256SUMS.
# The app has no Apple notarization, so after both checks pass the script
# removes the quarantine flag; the SSH signature is the trust anchor.
# Installs to ~/Applications/Ghostty Smooth.app (set GHOSTTY_SMOOTH_DEST to change).
set -eu

repo="tlpinney/ghostty"
namespace="ghostty-smooth-release"
signer="tlpinney"
asset="Ghostty-Smooth-macos-aarch64.zip"
dest_dir="${GHOSTTY_SMOOTH_DEST:-$HOME/Applications}"
# Same key as smooth-scroll/signing/allowed_signers, embedded so a downloaded
# script verifies against a key it already has.
allowed='tlpinney namespaces="ghostty-smooth-release" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFSZDt6uNCxuvSttHDBUcV1dctXJ1DBwYnQdbk19FfFP'

fail() { echo "ghostty-smooth: $*" >&2; exit 1; }

cat >&2 <<'BANNER'
Ghostty Smooth is an EXPERIMENTAL build: a personal fork of Ghostty that tries
out pixel smooth scrolling. It may be unstable, it is not an official Ghostty
release, and it is not affiliated with the Ghostty project.
BANNER

[ "$(uname -s)" = Darwin ] || fail "macOS only"
[ "$(uname -m)" = arm64 ] || fail "Apple Silicon only"
command -v ssh-keygen >/dev/null || fail "ssh-keygen is needed to verify the release"

if [ $# -ge 1 ]; then
  tag="$1"
else
  # GitHub's "latest" skips prereleases, and these are prereleases, so ask the
  # API for the newest published smooth-v release (drafts are not listed).
  tag="$(curl -fsSL "https://api.github.com/repos/$repo/releases?per_page=30" \
    | sed -n 's/^ *"tag_name": *"\(smooth-v[^"]*\)".*/\1/p' | head -1)"
  [ -n "$tag" ] || fail "could not find a published smooth-v release"
fi
base="https://github.com/$repo/releases/download/$tag"
echo "ghostty-smooth: installing $tag" >&2

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
for f in SHA256SUMS SHA256SUMS.sig "$asset"; do
  curl -fsSL -o "$tmp/$f" "$base/$f" || fail "could not download $f from $base"
done

printf '%s\n' "$allowed" > "$tmp/allowed_signers"
ssh-keygen -Y verify -f "$tmp/allowed_signers" -I "$signer" -n "$namespace" \
  -s "$tmp/SHA256SUMS.sig" < "$tmp/SHA256SUMS" >/dev/null 2>&1 \
  || fail "SHA256SUMS signature does not verify; not installing"

expected="$(awk -v f="$asset" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")"
[ -n "$expected" ] || fail "$asset is not listed in the signed SHA256SUMS"
actual="$(shasum -a 256 "$tmp/$asset" | cut -d' ' -f1)"
[ "$actual" = "$expected" ] || fail "$asset does not match SHA256SUMS; not installing"

ditto -x -k "$tmp/$asset" "$tmp/app"
app="$(find "$tmp/app" -maxdepth 1 -name '*.app' | head -1)"
[ -n "$app" ] || fail "no app in $asset"
codesign --verify "$app" || fail "the app's code signature is broken"

mkdir -p "$dest_dir"
target="$dest_dir/Ghostty Smooth.app"
if [ -e "$target" ]; then
  # Never overwrite a bundle in place: a running copy would be killed.
  mv "$target" "$HOME/.Trash/Ghostty Smooth.app.$(date +%s)"
fi
ditto "$app" "$target"
xattr -dr com.apple.quarantine "$target" 2>/dev/null || true
echo "ghostty-smooth: installed verified release to $target"
