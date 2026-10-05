#!/usr/bin/env bash
# 安全响应头与访问令牌轮换。
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

require_prerequisites
make_test_home ccswitch-security
PORT=15855

start_server "$PORT"

# header <名字> <curl 参数...>：取出某个响应头的值（大小写不敏感）
header() {
  local name="$1"; shift
  curl -sS -D - -o /dev/null "$@" \
    | tr -d '\r' \
    | awk -v want="$(printf '%s' "$name" | tr 'A-Z' 'a-z')" \
        'index(tolower($0), want ":")==1 { sub(/^[^:]*: */, ""); print; exit }'
}

echo "== 安全响应头（静态页面）"
check "X-Content-Type-Options" "nosniff" "$(header X-Content-Type-Options "$CC_BASE/")"
check "X-Frame-Options" "DENY" "$(header X-Frame-Options "$CC_BASE/")"
check "Referrer-Policy" "no-referrer" "$(header Referrer-Policy "$CC_BASE/")"
check "Permissions-Policy 存在" "True" \
  "$([ -n "$(header Permissions-Policy "$CC_BASE/")" ] && echo True || echo False)"

echo
echo "== 安全响应头（API 与错误响应也要有）"
check "API 响应带 nosniff" "nosniff" \
  "$(header X-Content-Type-Options -H "$CC_COOKIE" "$CC_BASE/api/env")"
check "401 响应带 nosniff" "nosniff" \
  "$(header X-Content-Type-Options "$CC_BASE/api/env")"
check "Host 被拒（400）时也带 nosniff" "nosniff" \
  "$(header X-Content-Type-Options -H 'Host: evil.test' "$CC_BASE/api/env")"
check "默认不下发 HSTS" "" \
  "$(header Strict-Transport-Security "$CC_BASE/")"

echo
echo "== 令牌轮换"
NEW_TOKEN="$(curl -sS -X POST "$CC_BASE/api/rotate-token" -H "$CC_COOKIE" \
  -H 'Content-Type: application/json' -d '{}' \
  | python3 -c 'import json,sys;print(json.load(sys.stdin)["token"])')"
check "生成了新令牌（128 位十六进制）" "64" "${#NEW_TOKEN}"
check "新令牌写进了 web-token 文件" "$NEW_TOKEN" \
  "$(cat "$CC_TEST_HOME/.cc-switch/web-token")"
check "旧令牌立即失效 → 401" "401" \
  "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$CC_BASE/api/invoke" \
    -H "$CC_COOKIE" -H 'Content-Type: application/json' -d '{"cmd":"get_settings","args":{}}')"
check "新令牌可用 → 200" "200" \
  "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$CC_BASE/api/invoke" \
    -H "Cookie: ccswitch_web_token=$NEW_TOKEN" -H 'Content-Type: application/json' \
    -d '{"cmd":"get_settings","args":{}}')"
# 轮换响应会顺手给当前浏览器换上新 cookie——新 cookie 必须与响应体里的新令牌一致
ROTATED_BODY="$(curl -sS -D /tmp/rotate-headers.$$ -X POST "$CC_BASE/api/rotate-token" \
  -H "Cookie: ccswitch_web_token=$NEW_TOKEN" -H 'Content-Type: application/json' -d '{}')"
check "轮换响应顺手换了 cookie" \
  "ccswitch_web_token=$(printf '%s' "$ROTATED_BODY" | python3 -c 'import json,sys;print(json.load(sys.stdin)["token"])')" \
  "$(tr -d '\r' < /tmp/rotate-headers.$$ | grep -i '^set-cookie:' | sed 's/^[Ss]et-[Cc]ookie: *//; s/;.*//')"
check "cookie 带 HttpOnly 与 SameSite" "True" \
  "$(tr -d '\r' < /tmp/rotate-headers.$$ | grep -i '^set-cookie:' | grep -qi 'httponly' \
     && tr -d '\r' < /tmp/rotate-headers.$$ | grep -i '^set-cookie:' | grep -qi 'samesite=strict' \
     && echo True || echo False)"
rm -f /tmp/rotate-headers.$$

CUSTOM="e2e-custom-token-0123456789"
check "可以指定令牌" "True" \
  "$(curl -sS -X POST "$CC_BASE/api/rotate-token" -H "Cookie: ccswitch_web_token=$(cat "$CC_TEST_HOME/.cc-switch/web-token")" \
    -H 'Content-Type: application/json' -d "{\"token\":\"$CUSTOM\"}" \
    | python3 -c 'import json,sys;print(json.load(sys.stdin)["token"]=="'"$CUSTOM"'")')"
check "指定令牌已生效" "200" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "Cookie: ccswitch_web_token=$CUSTOM" "$CC_BASE/api/env")"
check "太短的令牌被拒 → 400" "400" \
  "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$CC_BASE/api/rotate-token" \
    -H "Cookie: ccswitch_web_token=$CUSTOM" -H 'Content-Type: application/json' -d '{"token":"short"}')"
check "空令牌视为「随机生成」" "64" \
  "$(curl -sS -X POST "$CC_BASE/api/rotate-token" -H "Cookie: ccswitch_web_token=$CUSTOM" \
    -H 'Content-Type: application/json' -d '{"token":"   "}' \
    | python3 -c 'import json,sys;print(len(json.load(sys.stdin)["token"]))')"

echo
echo "== 重启后沿用轮换过的令牌（顺带验证 --hsts）"
ROTATED="$(cat "$CC_TEST_HOME/.cc-switch/web-token")"
kill "$CC_SERVER_PID" 2>/dev/null
wait "$CC_SERVER_PID" 2>/dev/null
CC_SERVER_PID=""
start_server "$PORT" --hsts
check "重启后读到的就是轮换后的令牌" "$ROTATED" "$CC_TOKEN"
check "重启后能用它调 API → 200" "200" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "$CC_COOKIE" "$CC_BASE/api/env")"
check "--hsts 下才下发 HSTS" "max-age=31536000" \
  "$(header Strict-Transport-Security "$CC_BASE/")"

echo
echo "== --no-token 下没有可轮换的令牌"
kill "$CC_SERVER_PID" 2>/dev/null
wait "$CC_SERVER_PID" 2>/dev/null
CC_SERVER_PID=""
start_server "$PORT" --no-token
check "轮换被拒 → 400" "400" \
  "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$CC_BASE/api/rotate-token" \
    -H 'Content-Type: application/json' -d '{}')"

summary
