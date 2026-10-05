#!/usr/bin/env bash
# 启动旗标与访问控制：--bind / --allow-host / --no-token 的安全组合。
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

require_prerequisites
make_test_home ccswitch-flags

echo "== 非回环 + --no-token 必须被拒绝"
OUT=$(HOME="$CC_TEST_HOME" "$SERVER_BIN" --bind 0.0.0.0 --no-token --port 15860 2>&1 | head -1)
check "拒绝启动" "True" \
  "$(printf '%s' "$OUT" | grep -q '拒绝启动' && echo True || echo False)"

echo
echo "== --allow-host：白名单内的主机放行、其它仍拒绝"
start_server 15861 --allow-host cc-switch.lan
check "白名单主机 → 200" "200" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H 'Host: cc-switch.lan:15861' -H "$CC_COOKIE" "$CC_BASE/api/env")"
check "白名单带端口也认 → 200" "200" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H 'Host: cc-switch.lan' -H "$CC_COOKIE" "$CC_BASE/api/env")"
check "未列出的主机 → 400" "400" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H 'Host: other.test:15861' "$CC_BASE/api/env")"
check "回环始终可用 → 200" "200" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "$CC_COOKIE" "$CC_BASE/api/env")"

summary
