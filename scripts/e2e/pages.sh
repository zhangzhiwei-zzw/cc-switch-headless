#!/usr/bin/env bash
# MCP / Skills / Prompts / 用量四页：验证命令分发真的接上了 service 层，
# 并且落到磁盘 / 数据库上（而不是只回了个 ok）。
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

require_prerequisites
make_test_home ccswitch-pages
PORT=15854

# Claude 被判定为「未安装」时，MCP 同步会**故意跳过**不创建任何文件；
# 这里先放一个空的配置目录，让同步路径是活的。
mkdir -p "$CC_TEST_HOME/.claude"
echo '{}' > "$CC_TEST_HOME/.claude/settings.json"

# 累加模式应用（OpenCode / Pi）的 live 配置要在启动**之前**就位：要验证的正是
# 「服务端启动时会把它同步进数据库」（桌面版一直这么做，服务端曾漏掉这一段）。
mkdir -p "$CC_TEST_HOME/.config/opencode"
cat > "$CC_TEST_HOME/.config/opencode/opencode.json" <<'JSON'
{
  "provider": {
    "opencode-go": {
      "npm": "@ai-sdk/openai-compatible",
      "options": {
        "baseURL": "https://opencode.example.invalid/v1",
        "apiKey": "{env:OPENCODE_API_KEY}"
      }
    }
  }
}
JSON

# Pi 更依赖这段启动同步：它连「从 live 导入」的命令都没有（桌面版也没有），
# 原生配置只能在启动时同步进数据库。
mkdir -p "$CC_TEST_HOME/.pi/agent"
cat > "$CC_TEST_HOME/.pi/agent/models.json" <<'JSON'
{
  "providers": {
    "pi-e2e": {
      "baseUrl": "https://pi.example.invalid/v1",
      "apiKey": "sk-e2e"
    }
  }
}
JSON

start_server "$PORT"

# jget <命令 json> <作用于整个响应的 python 表达式>
# 例：jget '{"cmd":"get_mcp_servers"}' 'len(d["data"])'
jget() {
  call "$1" | python3 -c "import json,sys;d=json.load(sys.stdin);print($2)"
}

echo "== Pi 状态命令"
check "Pi 原生供应商可读取" "True" \
  "$(jget '{"cmd":"get_pi_current_state"}' '"pi-e2e" in d["data"]["enabledProviderIds"]')"
check "Pi 默认供应商未设置" "None" \
  "$(jget '{"cmd":"get_pi_current_state"}' 'd["data"]["defaultProviderId"]')"
check "Pi 会话目录可发现" "available" \
  "$(jget '{"cmd":"get_pi_session_discovery"}' 'd["data"]["status"]')"
check "Pi 用量脚本可保存" "True" \
  "$(jget '{"cmd":"update_pi_provider_usage_script","args":{"id":"pi-e2e","usageScript":{"enabled":false,"language":"javascript","code":"return []"}}}' 'd["data"]')"
check "Pi 用量脚本读回一致" "return []" \
  "$(jget '{"cmd":"get_providers","args":{"app":"pi"}}' 'd["data"]["pi-e2e"]["meta"]["usage_script"]["code"]')"
# 损坏原生配置时必须返回读取错误，不得伪装为空供应商列表。
printf '{invalid' > "$CC_TEST_HOME/.pi/agent/models.json"
check "Pi 配置损坏时报告失败" "False" \
  "$(jget '{"cmd":"get_pi_current_state"}' 'd["ok"]')"
printf '{"providers":{"pi-e2e":{"baseUrl":"https://pi.example.invalid/v1","apiKey":"sk-e2e"}}}' > "$CC_TEST_HOME/.pi/agent/models.json"

echo
echo "== 能力表"
CAPS="$(curl -s "$CC_BASE/api/capabilities" -H "$CC_COOKIE")"
for feature in pageMcp pageSkills pagePrompts pageUsage; do
  check "$feature 已开启" "True" \
    "$(printf '%s' "$CAPS" | python3 -c "import json,sys;print(json.load(sys.stdin)['features']['$feature'])")"
done
check "openInFileManager 关闭" "False" \
  "$(printf '%s' "$CAPS" | python3 -c "import json,sys;print(json.load(sys.stdin)['features']['openInFileManager'])")"

echo
echo "== 自定义图标库"
# 合法的最小 PNG（1x1 RGBA）；service 会完整解码，长度和宽高都在上限内。
ICON_DATA="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII="
check "初始为空" "0" "$(jget '{"cmd":"get_custom_icons"}' 'len(d["data"])')"
check "上传一张 1x1 PNG" "False" \
  "$(jget "{\"cmd\":\"add_custom_icon\",\"args\":{\"name\":\"E2E icon\",\"data\":\"$ICON_DATA\"}}" 'd["data"]["existed"]')"
check "读回 1 个" "1" "$(jget '{"cmd":"get_custom_icons"}' 'len(d["data"])')"
check "名字正确" "E2E icon" "$(jget '{"cmd":"get_custom_icons"}' 'd["data"][0]["name"]')"
check "重命名" "E2E renamed" \
  "$(jget "{\"cmd\":\"rename_custom_icon\",\"args\":{\"id\":\"$(jget '{"cmd":"get_custom_icons"}' 'd["data"][0]["id"]')\",\"name\":\"E2E renamed\"}}" 'd["data"]["name"]')"
check "删除" "None" \
  "$(jget "{\"cmd\":\"delete_custom_icon\",\"args\":{\"id\":\"$(jget '{"cmd":"get_custom_icons"}' 'd["data"][0]["id"]')\"}}" 'd["data"]')"
check "已清空" "0" "$(jget '{"cmd":"get_custom_icons"}' 'len(d["data"])')"

echo
echo "== MCP"
check "初始没有服务器" "0" "$(jget '{"cmd":"get_mcp_servers"}' 'len(d["data"])')"
check "写入一个服务器" "None" \
  "$(jget '{"cmd":"upsert_mcp_server","args":{"server":{"id":"e2e","name":"E2E","server":{"command":"echo","args":["hi"]},"apps":{"claude":true,"codex":false,"gemini":false,"opencode":false,"openclaw":false,"hermes":false,"pi":false},"tags":[]}}}' 'd["data"]')"
check "读回 1 个" "1" "$(jget '{"cmd":"get_mcp_servers"}' 'len(d["data"])')"
check "名字正确" "E2E" "$(jget '{"cmd":"get_mcp_servers"}' 'd["data"]["e2e"]["name"]')"
check "落进了 ~/.claude.json 的 mcpServers" "e2e" \
  "$(python3 -c 'import json,sys;print(list(json.load(open(sys.argv[1]))["mcpServers"])[0])' \
    "$CC_TEST_HOME/.claude.json")"
check "validate_mcp_command 认得 sh" "True" \
  "$(jget '{"cmd":"validate_mcp_command","args":{"cmd":"sh"}}' 'd["data"]')"
check "删除" "True" "$(jget '{"cmd":"delete_mcp_server","args":{"id":"e2e"}}' 'd["data"]')"
check "已从 ~/.claude.json 移除" "0" \
  "$(python3 -c 'import json,sys;print(len(json.load(open(sys.argv[1]))["mcpServers"]))' \
    "$CC_TEST_HOME/.claude.json")"

echo
echo "== Prompts"
check "初始为空" "0" "$(jget '{"cmd":"get_prompts","args":{"app":"claude"}}' 'len(d["data"])')"
check "写入一条" "None" \
  "$(jget '{"cmd":"upsert_prompt","args":{"app":"claude","id":"p1","prompt":{"id":"p1","name":"E2E 提示词","content":"你好","enabled":false}}}' 'd["data"]')"
check "读回 1 条" "1" "$(jget '{"cmd":"get_prompts","args":{"app":"claude"}}' 'len(d["data"])')"
check "内容正确" "你好" "$(jget '{"cmd":"get_prompts","args":{"app":"claude"}}' 'd["data"]["p1"]["content"]')"
check "删除" "None" "$(jget '{"cmd":"delete_prompt","args":{"app":"claude","id":"p1"}}' 'd["data"]')"
check "已清空" "0" "$(jget '{"cmd":"get_prompts","args":{"app":"claude"}}' 'len(d["data"])')"
check "目标文件位置以 CLAUDE.md 结尾" "True" \
  "$(jget '{"cmd":"get_prompt_file_location","args":{"app":"claude"}}' 'd["data"]["path"].endswith("CLAUDE.md")')"

check "重新写入并启用" "None" \
  "$(jget '{"cmd":"upsert_prompt","args":{"app":"claude","id":"p1","prompt":{"id":"p1","name":"E2E 提示词","content":"你好","enabled":false}}}' 'd["data"]')"
check "启用" "None" "$(jget '{"cmd":"enable_prompt","args":{"app":"claude","id":"p1"}}' 'd["data"]')"
check "启用后内容落到 CLAUDE.md" "你好" \
  "$(python3 -c 'import pathlib;print(pathlib.Path("'"$CC_TEST_HOME"'/.claude/CLAUDE.md").read_text().strip())')"
check "已启用的提示词不能删除" "False" \
  "$(jget '{"cmd":"delete_prompt","args":{"app":"claude","id":"p1"}}' 'd["ok"]')"

echo
echo "== Skills"
check "初始没有已安装 Skill" "0" "$(jget '{"cmd":"get_installed_skills"}' 'len(d["data"])')"
check "添加仓库" "True" \
  "$(jget '{"cmd":"add_skill_repo","args":{"repo":{"owner":"anthropics","name":"skills","branch":"main","enabled":true}}}' 'd["data"]')"
check "读回该仓库" "True" \
  "$(jget '{"cmd":"get_skill_repos"}' 'any(r["owner"]=="anthropics" and r["name"]=="skills" for r in d["data"])')"
check "非法 branch 被拒" "False" \
  "$(jget '{"cmd":"add_skill_repo","args":{"repo":{"owner":"anthropics","name":"skills","branch":"../../evil","enabled":true}}}' 'd["ok"]')"
check "存储目录指向配置目录下的 skills/" "True" \
  "$(jget '{"cmd":"get_cc_switch_skills_dir"}' 'd["data"].endswith("/.cc-switch/skills")')"
check "删除仓库" "True" \
  "$(jget '{"cmd":"remove_skill_repo","args":{"owner":"anthropics","name":"skills"}}' 'd["data"]')"
check "已移除" "False" \
  "$(jget '{"cmd":"get_skill_repos"}' 'any(r["owner"]=="anthropics" and r["name"]=="skills" for r in d["data"])')"

check "未配置的单仓库发现返回空结果" "True" \
  "$(jget '{"cmd":"discover_repo_skills","args":{"owner":"cc-switch-e2e","name":"not-configured"}}' 'd["ok"] and d["data"] == {"skills":[],"failures":[]}')"
check "添加停用仓库" "True" \
  "$(jget '{"cmd":"add_skill_repo","args":{"repo":{"owner":"cc-switch-e2e","name":"disabled","branch":"main","enabled":false}}}' 'd["data"]')"
check "停用仓库发现不触发网络下载" "True" \
  "$(jget '{"cmd":"discover_repo_skills","args":{"owner":"cc-switch-e2e","name":"disabled"}}' 'd["ok"] and d["data"] == {"skills":[],"failures":[]}')"
check "非法刷新参数被拒" "False" \
  "$(jget '{"cmd":"discover_available_skills","args":{"forceRefresh":"invalid"}}' 'd["ok"]')"
check "删除停用测试仓库" "True" \
  "$(jget '{"cmd":"remove_skill_repo","args":{"owner":"cc-switch-e2e","name":"disabled"}}' 'd["data"]')"

echo
echo "== 用量图表设置在服务端持久化"
call '{"cmd":"get_settings"}' | python3 -c '
import json,sys
settings=json.load(sys.stdin)["data"]
settings.setdefault("usageDisplay", {})["trendChartType"]="area"
print(json.dumps({"cmd":"save_settings","args":{"settings":settings}}))
' > "$CC_TEST_HOME/chart-settings-request.json"
check "保存面积图偏好" "True" \
  "$(jget "$(cat "$CC_TEST_HOME/chart-settings-request.json")" 'd["ok"]')"
check "再次读取面积图偏好" "area" \
  "$(jget '{"cmd":"get_settings"}' 'd["data"]["usageDisplay"]["trendChartType"]')"
check "面积图偏好实际写入磁盘" "area" \
  "$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["usageDisplay"]["trendChartType"])' "$CC_TEST_HOME/.cc-switch/settings.json")"

echo
echo "== 用量"
check "模型目录与汇率命令已注册" "True" \
  "$(curl -fsS -H "$CC_COOKIE" "$CC_BASE/api/commands" | python3 -c 'import json,sys; names=json.load(sys.stdin)["commands"]; print(all(x in names for x in ["fetch_models_dev","fetch_exchange_rates"]))')"
check "汇总可用（空库为 0）" "0" "$(jget '{"cmd":"get_usage_summary","args":{}}' 'd["data"]["totalRequests"]')"
check "按应用拆分为空" "0" "$(jget '{"cmd":"get_usage_summary_by_app","args":{}}' 'len(d["data"])')"
check "趋势里没有请求" "0" \
  "$(jget '{"cmd":"get_usage_trends","args":{}}' 'sum(x["requestCount"] for x in d["data"])')"
check "请求日志第 0 页" "0" \
  "$(jget '{"cmd":"get_request_logs","args":{"filters":{},"page":0,"pageSize":20}}' 'len(d["data"]["data"])')"
check "内置模型定价已播种" "True" "$(jget '{"cmd":"get_model_pricing"}' 'len(d["data"]) > 0')"
check "写入一条定价" "None" \
  "$(jget '{"cmd":"update_model_pricing","args":{"modelId":"e2e-model","displayName":"E2E Model","inputCost":"1","outputCost":"2","cacheReadCost":"0.1","cacheCreationCost":"0.2"}}' 'd["data"]')"
check "读回该条" "E2E Model" \
  "$(jget '{"cmd":"get_model_pricing"}' '[m["displayName"] for m in d["data"] if m["modelId"]=="e2e-model"][0]')"
check "删除该条" "None" \
  "$(jget '{"cmd":"delete_model_pricing","args":{"modelId":"e2e-model"}}' 'd["data"]')"
check "已删除" "0" \
  "$(jget '{"cmd":"get_model_pricing"}' 'len([m for m in d["data"] if m["modelId"]=="e2e-model"])')"
check "计费来源默认 response" "response" \
  "$(jget '{"cmd":"get_pricing_model_source","args":{"appType":"claude"}}' 'd["data"]')"
check "切换计费来源" "None" \
  "$(jget '{"cmd":"set_pricing_model_source","args":{"appType":"claude","value":"request"}}' 'd["data"]')"
check "切换已生效" "request" \
  "$(jget '{"cmd":"get_pricing_model_source","args":{"appType":"claude"}}' 'd["data"]')"
check "非法计费来源被拒" "False" \
  "$(jget '{"cmd":"set_pricing_model_source","args":{"appType":"claude","value":"nope"}}' 'd["ok"]')"
check "数据来源分布可查" "0" "$(jget '{"cmd":"get_usage_data_sources"}' 'len(d["data"])')"
check "会话同步可跑（临时 HOME 无会话）" "0" \
  "$(jget '{"cmd":"sync_session_usage"}' 'd["data"]["imported"]')"

echo
echo "== 累加模式应用（OpenCode）"
# 这三条对应的是服务端曾经漏掉的那段：累加模式应用既不进通用导入
# （import_default_config 对它们直接返回 false），启动时也要各自同步 live。
check "启动时已从 live 同步进数据库" "True" \
  "$(jget '{"cmd":"get_providers","args":{"app":"opencode"}}' '"opencode-go" in d["data"]')"
check "再次导入是幂等的（返回 0）" "0" \
  "$(jget '{"cmd":"import_opencode_providers_from_live"}' 'd["data"]')"
check "live provider ids 可读" "1" \
  "$(jget '{"cmd":"get_opencode_live_provider_ids"}' 'len(d["data"])')"
check "Pi 也同步了（它只有启动同步这一条路）" "True" \
  "$(jget '{"cmd":"get_providers","args":{"app":"pi"}}' '"pi-e2e" in d["data"]')"

echo
echo "== DeepSeek Harness 用量导入"
python3 - "$CC_TEST_HOME" <<'PY'
import json, pathlib, sys, time
root = pathlib.Path(sys.argv[1])
path = root / '.dsh/sessions/e2e/project/session.v4.jsonl'
path.parent.mkdir(parents=True, exist_ok=True)
now = int(time.time() * 1000)
records = [
    {"type":"session", "version":4, "id":"e2e-dsh", "cwd":"/e2e", "createdAt":now},
    {"type":"assistant/message", "time":now, "data":{
        "turn":1,"step":1,
        "message":{"id":"e2e-dsh-message", "role":"assistant",
            "source":{"kind":"model", "provider":"deepseek-account", "model":"deepseek-flash"},
            "content":[{"type":"text", "text":"ok"}]},
        "usage":{"inputTokens":10,"outputTokens":5,"cacheReadTokens":20,"cacheWriteTokens":0}
    }}
]
path.write_text(''.join(json.dumps(record) + '\n' for record in records))
PY
check "DSH 会话同步接口成功" "True" \
  "$(jget '{"cmd":"sync_session_usage"}' 'd["ok"]')"
check "DSH 用量实际入库且保留 fresh input 语义" "1" \
  "$(python3 - "$CC_TEST_HOME/.cc-switch/cc-switch.db" <<'PY'
import sqlite3, sys
with sqlite3.connect(sys.argv[1]) as db:
    print(db.execute("SELECT COUNT(*) FROM proxy_request_logs WHERE app_type='dsh' AND input_tokens=10 AND output_tokens=5 AND cache_read_tokens=20").fetchone()[0])
PY
)"
check "重复同步不会重复导入 DSH 请求" "0" \
  "$(jget '{"cmd":"sync_session_usage"}' 'd["data"]["imported"]')"

echo
echo "== Oh My Pi 用量导入"
python3 - "$CC_TEST_HOME" <<'PY'
import datetime, json, pathlib, sys
root = pathlib.Path(sys.argv[1])
path = root / '.omp/agent/sessions/e2e/session.jsonl'
path.parent.mkdir(parents=True, exist_ok=True)
now = datetime.datetime.now(datetime.timezone.utc).isoformat()
records = [
    {"type":"title", "v":1, "title":"OMP usage", "updatedAt":now, "pad":""},
    {"type":"session", "version":3, "id":"e2e-omp-session", "timestamp":now, "cwd":"/e2e"},
    {"type":"message", "id":"e2e-omp-message", "parentId":None, "timestamp":now, "message":{
        "role":"assistant", "content":[], "api":"openai-completions", "provider":"e2e", "model":"e2e-omp-model",
        "duration":1234, "ttft":200,
        "usage":{"input":7,"output":3,"cacheRead":2,"cacheWrite":0,"totalTokens":12,
            "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
        "stopReason":"stop"
    }}
]
path.write_text(''.join(json.dumps(record) + '\n' for record in records))
PY
check "OMP 会话同步接口成功" "True" \
  "$(jget '{"cmd":"sync_session_usage"}' 'd["ok"]')"
# 上游 OMP 导入器只记录总延迟；原生 ttft 尚未导入，首字时间保持为空。
check "OMP 用量和延迟实际入库" "1" \
  "$(python3 - "$CC_TEST_HOME/.cc-switch/cc-switch.db" <<'PY'
import sqlite3, sys
with sqlite3.connect(sys.argv[1]) as db:
    print(db.execute("SELECT COUNT(*) FROM proxy_request_logs WHERE app_type='omp' AND data_source='omp_session' AND input_tokens=7 AND output_tokens=3 AND latency_ms=1234 AND first_token_ms IS NULL").fetchone()[0])
PY
)"
check "OMP 会话不会被 Pi 重复导入" "0" \
  "$(python3 - "$CC_TEST_HOME/.cc-switch/cc-switch.db" <<'PY'
import sqlite3, sys
with sqlite3.connect(sys.argv[1]) as db:
    print(db.execute("SELECT COUNT(*) FROM proxy_request_logs WHERE app_type='pi'").fetchone()[0])
PY
)"
check "重复同步不会重复导入 OMP 请求" "0" \
  "$(jget '{"cmd":"sync_session_usage"}' 'd["data"]["imported"]')"

echo
echo "== 新应用与 Skills 元数据刷新"
check "无 Skills 时刷新元数据返回 0" "0" \
  "$(jget '{"cmd":"refresh_local_skill_metadata"}' 'd["data"]')"
check "Copilot BYOK 卡可保存" "True" \
  "$(jget '{"cmd":"add_provider","args":{"app":"copilot","provider":{"id":"web-relay","name":"Web relay","category":"custom","settingsConfig":{"provider":{"type":"openai","baseUrl":"https://example.invalid/v1","apiKey":"fixture-key"},"models":[{"id":"test-model"}]}}}}' 'd["ok"]')"
check "Copilot 供应商实际写入 providers.json" "True" \
  "$(python3 - "$CC_TEST_HOME/.copilot/providers.json" <<'PYINNER'
import json,sys
config=json.load(open(sys.argv[1]))
print(any(p.get('name') == 'web-relay' for p in config['providers']) and any(m.get('id') == 'test-model' and m.get('provider') == 'web-relay' for m in config['models']))
PYINNER
)"
check "WorkBuddy 自定义模型可保存" "True" \
  "$(jget '{"cmd":"add_provider","args":{"app":"workbuddy","provider":{"id":"web-buddy","name":"Web buddy","category":"custom","settingsConfig":{"url":"https://example.invalid/v1/chat/completions","apiKey":"fixture-key","models":[{"id":"buddy-model"}]}}}}' 'd["ok"]')"
check "WorkBuddy 模型归属可读取" "True" \
  "$(jget '{"cmd":"workbuddy_model_owners"}' 'any(m["id"] == "buddy-model" for m in d["data"])')"
check "WorkBuddy 模型配置实际落盘" "True" \
  "$(python3 - "$CC_TEST_HOME/.workbuddy/models.json" <<'PYINNER'
import json,sys
print('buddy-model' in json.dumps(json.load(open(sys.argv[1]))))
PYINNER
)"

summary
