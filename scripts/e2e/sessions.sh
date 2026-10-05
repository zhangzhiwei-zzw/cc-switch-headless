#!/usr/bin/env bash
# 会话浏览：在临时 HOME 里造一个 Claude Code 会话文件，验证 列表 → 读取 → 整段 → 删除。
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

require_prerequisites
make_test_home ccswitch-sessions
PORT=15853

SESSION_ID="11111111-2222-3333-4444-555555555555"
PROJECT="$CC_TEST_HOME/.claude/projects/-home-e2e-project"
mkdir -p "$PROJECT"
cat > "$PROJECT/$SESSION_ID.jsonl" <<'JSONL'
{"sessionId":"11111111-2222-3333-4444-555555555555","cwd":"/home/e2e/project","timestamp":"2026-10-05T00:00:00.000Z","type":"user","message":{"role":"user","content":"e2e：请列出一个目录"}}
{"sessionId":"11111111-2222-3333-4444-555555555555","cwd":"/home/e2e/project","timestamp":"2026-10-05T00:00:02.000Z","type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"好的，这是结果。"}]}}
{"sessionId":"11111111-2222-3333-4444-555555555555","cwd":"/home/e2e/project","timestamp":"2026-10-05T00:00:03.000Z","type":"user","message":{"role":"user","content":"谢谢"}}
JSONL

start_server "$PORT"

echo "== 列表"
check "扫到 1 个会话" "1" \
  "$(call '{"cmd":"list_sessions","args":{}}' | python3 -c 'import json,sys;print(len(json.load(sys.stdin)["data"]))')"
check "标题取自首条用户消息" "e2e：请列出一个目录" \
  "$(call '{"cmd":"list_sessions","args":{}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"][0]["title"])')"
check "带 resume 命令" "claude --resume $SESSION_ID" \
  "$(call '{"cmd":"list_sessions","args":{}}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"][0]["resumeCommand"])')"

echo
echo "== 读取"
check "get_session_messages 返回 3 条" "3" \
  "$(call "{\"cmd\":\"get_session_messages\",\"args\":{\"providerId\":\"claude\",\"sourcePath\":\"$PROJECT/$SESSION_ID.jsonl\"}}" | python3 -c 'import json,sys;print(len(json.load(sys.stdin)["data"]))')"
check "整段含 2 个轮次" "2" \
  "$(call "{\"cmd\":\"web_session_transcript\",\"args\":{\"providerId\":\"claude\",\"sourcePath\":\"$PROJECT/$SESSION_ID.jsonl\"}}" | python3 -c 'import json,sys;print(len(json.load(sys.stdin)["data"]["turns"]))')"

echo
echo "== 删除"
check "delete_session" "True" \
  "$(call "{\"cmd\":\"delete_session\",\"args\":{\"providerId\":\"claude\",\"sessionId\":\"$SESSION_ID\",\"sourcePath\":\"$PROJECT/$SESSION_ID.jsonl\"}}" | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"])')"
check "文件已删除" "False" "$([ -f "$PROJECT/$SESSION_ID.jsonl" ] && echo True || echo False)"
check "列表变空" "0" \
  "$(call '{"cmd":"list_sessions","args":{}}' | python3 -c 'import json,sys;print(len(json.load(sys.stdin)["data"]))')"

summary
