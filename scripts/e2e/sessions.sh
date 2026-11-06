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
echo "== 会话正文索引（默认关闭，按需启用）"
check "正文搜索默认没有命中" "0" \
  "$(call '{"cmd":"search_session_content","args":{"query":"结果"}}' | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["data"]["hits"]))')"
call '{"cmd":"get_settings"}' | python3 -c '
import json,sys
settings=json.load(sys.stdin)["data"]
settings["sessionContentSearchEnabled"]=True
print(json.dumps({"cmd":"save_settings","args":{"settings":settings}}))
' > "$CC_TEST_HOME/search-settings.json"
check "启用正文搜索" "True" \
  "$(call "$(cat "$CC_TEST_HOME/search-settings.json")" | python3 -c 'import json,sys; print(json.load(sys.stdin)["ok"])')"
# A discoverable source above the Web limit must not be parsed into the index.
LARGE_SESSION="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
python3 - "$PROJECT/$LARGE_SESSION.jsonl" "$LARGE_SESSION" <<'PYLARGE'
import json,sys
with open(sys.argv[1],"wb") as f:
    f.write((json.dumps({"sessionId":sys.argv[2],"cwd":"/home/e2e/project","timestamp":"2026-10-05T00:00:00.000Z","type":"user","message":{"role":"user","content":"index-oversize-marker"}})+"\n").encode())
    f.truncate(64*1024*1024+1)
PYLARGE
check "超限测试会话被扫描发现" "True" \
  "$(call '{"cmd":"list_sessions"}' | python3 -c 'import json,sys; print(any(s["sessionId"]=="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" for s in json.load(sys.stdin)["data"]))')"
check "启动正文索引" "True" \
  "$(call '{"cmd":"sync_session_index"}' | python3 -c 'import json,sys; print(json.load(sys.stdin)["ok"])')"
for attempt in $(seq 1 100); do
  SEARCH_RESULT=$(call '{"cmd":"search_session_content","args":{"query":"结果","providerIds":["claude"]}}')
  if python3 -c 'import json,sys; d=json.load(sys.stdin)["data"]; sys.exit(not (d["hits"] and d["status"]["total"]>=2 and not d["status"]["running"] and d["status"]["processed"]==d["status"]["total"]))' <<< "$SEARCH_RESULT"; then break; fi
  sleep 0.1
done
check "正文搜索命中消息并包含高亮" "True" \
  "$(python3 -c 'import json,sys; d=json.load(sys.stdin)["data"]; h=d["hits"][0]; print(h["providerId"]=="claude" and h["snippets"][0]["messageIndex"]==1 and bool(h["snippets"][0]["highlights"]))' <<< "$SEARCH_RESULT")"
check "应用过滤在正文搜索中生效" "0" \
  "$(call '{"cmd":"search_session_content","args":{"query":"结果","providerIds":["codex"]}}' | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["data"]["hits"]))')"
check "索引读取仍拒绝超过 64 MiB 的源" "0" \
  "$(call '{"cmd":"search_session_content","args":{"query":"index-oversize-marker"}}' | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["data"]["hits"]))')"
check "索引写入独立文件" "True" "$([ -f "$CC_TEST_HOME/.cc-switch/session-index.db" ] && echo True || echo False)"
call '{"cmd":"get_settings"}' | python3 -c '
import json,sys
settings=json.load(sys.stdin)["data"]
settings["sessionContentSearchEnabled"]=False
print(json.dumps({"cmd":"save_settings","args":{"settings":settings}}))
' > "$CC_TEST_HOME/search-settings.json"
check "关闭正文搜索" "True" \
  "$(call "$(cat "$CC_TEST_HOME/search-settings.json")" | python3 -c 'import json,sys; print(json.load(sys.stdin)["ok"])')"
check "清除正文索引" "True" \
  "$(call '{"cmd":"clear_session_index"}' | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"])')"
check "索引及 WAL 文件已移除" "True" \
  "$(python3 - "$CC_TEST_HOME/.cc-switch/session-index.db" <<'PYCLEAR'
from pathlib import Path
import sys
print(all(not Path(sys.argv[1]+suffix).exists() for suffix in ["","-wal","-shm"]))
PYCLEAR
)"
# Delete only the temporary oversized fixture so the existing delete assertions stay focused.
python3 - "$PROJECT/$LARGE_SESSION.jsonl" <<'PYREMOVE'
from pathlib import Path
import sys
Path(sys.argv[1]).unlink()
PYREMOVE

echo
echo "== 删除"
check "delete_session" "True" \
  "$(call "{\"cmd\":\"delete_session\",\"args\":{\"providerId\":\"claude\",\"sessionId\":\"$SESSION_ID\",\"sourcePath\":\"$PROJECT/$SESSION_ID.jsonl\"}}" | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"])')"
check "文件已删除" "False" "$([ -f "$PROJECT/$SESSION_ID.jsonl" ] && echo True || echo False)"
check "列表变空" "0" \
  "$(call '{"cmd":"list_sessions","args":{}}' | python3 -c 'import json,sys;print(len(json.load(sys.stdin)["data"]))')"

summary
