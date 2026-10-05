#!/usr/bin/env bash
# 本地路由：请求真的穿过代理到达上游，且进入/退出路由模式会改写/回退 CLI 配置。
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

require_prerequisites
make_test_home ccswitch-proxy

PORT=15851
UPSTREAM_PORT=19099
PROXY_PORT=15721
UPSTREAM_LOG="$CC_TEST_HOME/upstream.log"

mkdir -p "$CC_TEST_HOME/.claude"
echo '{"env":{"ANTHROPIC_BASE_URL":"https://old.example.invalid","ANTHROPIC_API_KEY":"sk-old"}}' \
  > "$CC_TEST_HOME/.claude/settings.json"

echo "== 启动假上游与本地服务端"
python3 "$(dirname "${BASH_SOURCE[0]}")/fake-upstream.py" "$UPSTREAM_PORT" "$UPSTREAM_LOG" &
CC_UPSTREAM_PID=$!
start_server "$PORT"

echo
echo "== 配一个指向假上游的供应商并切换"
call '{"cmd":"add_provider","args":{"app":"claude","provider":{"id":"fake-upstream","name":"Fake Upstream","settingsConfig":{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:'"$UPSTREAM_PORT"'","ANTHROPIC_API_KEY":"sk-fake-upstream"}},"category":"custom"}}}' >/dev/null
call '{"cmd":"switch_provider","args":{"app":"claude","id":"fake-upstream"}}' >/dev/null
check "当前供应商" "fake-upstream" \
  "$(call '{"cmd":"get_current_provider","args":{"app":"claude"}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"])')"

echo
echo "== 启动代理并进入路由模式"
check "start_proxy_server" "True" \
  "$(call '{"cmd":"start_proxy_server","args":{}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["ok"])')"
call '{"cmd":"set_proxy_takeover_for_app","args":{"appType":"claude","enabled":true}}' >/dev/null
check "get_app_mode = route" "route" \
  "$(call '{"cmd":"get_app_mode","args":{"appType":"claude"}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"]["mode"])')"

echo
echo "== CLI 配置被改写到本地代理"
check "ANTHROPIC_BASE_URL 指向代理" "http://127.0.0.1:$PROXY_PORT" \
  "$(python3 -c 'import json;print(json.load(open("'"$CC_TEST_HOME"'/.claude/settings.json"))["env"]["ANTHROPIC_BASE_URL"])')"

echo
echo "== 发一个 Anthropic 请求穿过代理"
PROXY_AUTH=$(python3 -c '
import json
env = json.load(open("'"$CC_TEST_HOME"'/.claude/settings.json"))["env"]
print(env.get("ANTHROPIC_AUTH_TOKEN") or env.get("ANTHROPIC_API_KEY") or "")')
HTTP_CODE=$(curl -s -o "$CC_TEST_HOME/proxy-response.json" -w '%{http_code}' \
  -X POST "http://127.0.0.1:$PROXY_PORT/v1/messages" \
  -H 'Content-Type: application/json' -H 'anthropic-version: 2023-06-01' \
  -H "x-api-key: $PROXY_AUTH" \
  -d '{"model":"claude-sonnet-5","max_tokens":16,"messages":[{"role":"user","content":"ping"}]}')
check "代理返回 200" "200" "$HTTP_CODE"
check "响应来自假上游" "pong from fake upstream" \
  "$(python3 -c 'import json;print(json.load(open("'"$CC_TEST_HOME"'/proxy-response.json"))["content"][0]["text"])' 2>/dev/null || echo "解析失败")"

check "上游收到 /v1/messages" "/v1/messages" \
  "$(python3 -c 'import json;print(json.loads(open("'"$UPSTREAM_LOG"'").readline())["path"])' 2>/dev/null || echo "上游没收到")"
check "上游收到供应商的 key" "sk-fake-upstream" \
  "$(python3 -c 'import json;print(json.loads(open("'"$UPSTREAM_LOG"'").readline())["x-api-key"])' 2>/dev/null || echo "-")"

echo
echo "== 退出路由模式后配置回退"
call '{"cmd":"set_proxy_takeover_for_app","args":{"appType":"claude","enabled":false}}' >/dev/null
check "回退到直连供应商" "http://127.0.0.1:$UPSTREAM_PORT" \
  "$(python3 -c 'import json;print(json.load(open("'"$CC_TEST_HOME"'/.claude/settings.json"))["env"]["ANTHROPIC_BASE_URL"])')"

summary
