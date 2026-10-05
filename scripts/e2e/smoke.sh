#!/usr/bin/env bash
# 冒烟：启动链路 + 供应商增删切换 + 真实落盘 + 安全校验
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

require_prerequisites
make_test_home ccswitch-smoke
PORT=15850

mkdir -p "$CC_TEST_HOME/.claude"
cat > "$CC_TEST_HOME/.claude/settings.json" <<'JSON'
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://old.example.invalid",
    "ANTHROPIC_API_KEY": "sk-old-key"
  },
  "permissions": { "allow": ["Bash(ls:*)"] }
}
JSON

echo "== 启动服务端（端口 $PORT）"
start_server "$PORT"

echo
echo "== 静态页面与元信息"
check "GET / 可访问" "200" "$(curl -s -o /dev/null -w '%{http_code}' "$CC_BASE/")"
check "/api/env 带令牌可访问" "200" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "$CC_COOKIE" "$CC_BASE/api/env")"
check "/api/capabilities 可访问" "200" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "$CC_COOKIE" "$CC_BASE/api/capabilities")"

echo
echo "== 启动链路命令"
check "get_init_error" "null" "$(call '{"cmd":"get_init_error","args":{}}' | python3 -c 'import json,sys;print(json.dumps(json.load(sys.stdin)["data"]))')"
check "get_settings 返回对象" "True" "$(call '{"cmd":"get_settings","args":{}}' | python3 -c 'import json,sys;print(isinstance(json.load(sys.stdin).get("data"),dict))')"

echo
echo "== 供应商"
check "首次启动导入了 live 配置" "default" \
  "$(call '{"cmd":"get_current_provider","args":{"app":"claude"}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"])')"
check "add_provider" "True" \
  "$(call '{"cmd":"add_provider","args":{"app":"claude","provider":{"id":"e2e-smoke","name":"E2E Smoke","settingsConfig":{"env":{"ANTHROPIC_BASE_URL":"https://smoke.example.com","ANTHROPIC_API_KEY":"sk-smoke"}},"category":"custom"}}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"])')"
check "switch_provider" "e2e-smoke" \
  "$(call '{"cmd":"switch_provider","args":{"app":"claude","id":"e2e-smoke"}}' >/dev/null; call '{"cmd":"get_current_provider","args":{"app":"claude"}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"])')"

echo
echo "== 真实落盘（且保留用户自己的配置）"
check "settings.json 已指向新供应商" "https://smoke.example.com" \
  "$(python3 -c 'import json;print(json.load(open("'"$CC_TEST_HOME"'/.claude/settings.json"))["env"]["ANTHROPIC_BASE_URL"])')"
check "用户自己的 permissions 保留" "1" \
  "$(grep -c 'Bash(ls' "$CC_TEST_HOME/.claude/settings.json")"

echo
echo "== 安全"
check "无令牌 → 401" "401" \
  "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$CC_BASE/api/invoke" -H 'Content-Type: application/json' -d '{"cmd":"get_providers","args":{"app":"claude"}}')"
check "伪造 Host → 400" "400" "$(curl -s -o /dev/null -w '%{http_code}' -H 'Host: evil.test' "$CC_BASE/api/env")"
check "跨站 Origin → 403" "403" "$(curl -s -o /dev/null -w '%{http_code}' -H 'Origin: http://evil.test' "$CC_BASE/api/env")"
check "未实现命令 → E_NOT_IMPLEMENTED" "True" \
  "$(call '{"cmd":"query_provider_usage","args":{}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["error"].startswith("E_NOT_IMPLEMENTED"))')"

summary
