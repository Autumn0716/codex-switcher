#!/usr/bin/env bash
set -euo pipefail

cd /workspace

npm ci
npm run linux:build

mkdir -p /workspace/target/linux-release
find /workspace/src-tauri/target/release/bundle -type f \
  \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
  -exec cp {} /workspace/target/linux-release/ \;

printf '\nLinux artifacts:\n'
ls -lh /workspace/target/linux-release
