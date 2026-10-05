#!/usr/bin/env bash
# 依次跑完所有端到端用例。
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FAILED=()

for script in smoke.sh flags.sh proxy.sh import-export.sh sessions.sh pages.sh; do
  echo "════════════════════════════════════════"
  echo "  $script"
  echo "════════════════════════════════════════"
  if ! bash "$DIR/$script"; then
    FAILED+=("$script")
  fi
  echo
done

if [ "${#FAILED[@]}" -eq 0 ]; then
  echo "── 全部用例通过 ──"
  exit 0
fi

echo "── 失败用例：${FAILED[*]} ──"
exit 1
