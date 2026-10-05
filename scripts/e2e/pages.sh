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

start_server "$PORT"

# jget <命令 json> <作用于整个响应的 python 表达式>
# 例：jget '{"cmd":"get_mcp_servers"}' 'len(d["data"])'
jget() {
  call "$1" | python3 -c "import json,sys;d=json.load(sys.stdin);print($2)"
}

echo "== 能力表"
CAPS="$(curl -s "$CC_BASE/api/capabilities" -H "$CC_COOKIE")"
for feature in pageMcp pageSkills pagePrompts pageUsage; do
  check "$feature 已开启" "True" \
    "$(printf '%s' "$CAPS" | python3 -c "import json,sys;print(json.load(sys.stdin)['features']['$feature'])")"
done
check "openInFileManager 关闭" "False" \
  "$(printf '%s' "$CAPS" | python3 -c "import json,sys;print(json.load(sys.stdin)['features']['openInFileManager'])")"

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

echo
echo "== 用量"
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

summary
