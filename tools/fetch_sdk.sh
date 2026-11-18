#!/usr/bin/env bash
# Downloads and unpacks the Linux client slices of the 2d81263 SDK into $JADE_SDK/pkg, checked
# against the release's SHA256SUMS, and its cargo-sunrise-mod tool into $JADE_SDK/tool.
set -euo pipefail
cd "$(dirname "$0")/.."
. tools/env.sh
BUILD=2d8126336e579777eabf9b2fd8b3d476236a04f2
BASE="https://github.com/sunrise-org/sunrise/releases/download/sdk-$BUILD"
mkdir -p "$JADE_SDK/dl" "$JADE_SDK/pkg"
cd "$JADE_SDK"
curl -sSfL -o SHA256SUMS "$BASE/SHA256SUMS"
for f in tool-x86_64-unknown-linux-gnu.tar.xz api-x86_64-unknown-linux-gnu.tar.xz runtime-x86_64-unknown-linux-gnu.tar.xz engine-x86_64-unknown-linux-gnu.tar.xz; do
  [ -f "dl/$f" ] || curl -sSfL -o "dl/$f" "$BASE/$f"
  (cd dl && grep " $f\$" ../SHA256SUMS | sha256sum -c -)
done
mkdir -p tool && tar -xJf dl/tool-x86_64-unknown-linux-gnu.tar.xz -C tool
for s in api runtime engine; do
  [ -d "pkg/$s" ] || { mkdir -p "pkg/$s" && tar -xJf "dl/$s-x86_64-unknown-linux-gnu.tar.xz" -C "pkg/$s"; }
done
echo "SDK slices in $JADE_SDK/pkg"
