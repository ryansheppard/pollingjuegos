#!/usr/bin/env bash
# Builds the desktop app for Linux and Windows and publishes them as a GitHub release.
# Runs on the runner MicroVM from the repo root; gh reads GH_TOKEN and infers the repo
# from the checkout's origin remote.
#
# Usage: runner/release-desktop.sh <tag>
set -euo pipefail

tag=${1:?usage: $0 <tag>}
name="pollingjuegos-desktop-$tag"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/dist" "$work/staging"

# bazel-bin follows the last build's configuration, so copy each
# output out before building the next platform.
bazel build //desktop -c opt
cp bazel-bin/desktop/desktop "$work/staging/pollingjuegos"
tar -C "$work/staging" -czf "$work/dist/$name-linux-x86_64.tar.gz" pollingjuegos

bazel build //desktop -c opt --platforms=//:windows_x86_64
cp bazel-bin/desktop/desktop.exe "$work/staging/pollingjuegos.exe"
(cd "$work/staging" && zip -q "$work/dist/$name-windows-x86_64.zip" pollingjuegos.exe)

(cd "$work/dist" && sha256sum -- * >SHA256SUMS)

gh release create "$tag" "$work"/dist/* \
  --title "$tag" \
  --generate-notes \
  --verify-tag
