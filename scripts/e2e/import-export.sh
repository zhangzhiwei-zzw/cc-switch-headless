#!/usr/bin/env bash
# 导入导出与备份：导出 → 下载 → 上传 → 恢复 的完整回环，以及下载的目录越权防护。
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

require_prerequisites
make_test_home ccswitch-io
PORT=15852

mkdir -p "$CC_TEST_HOME/.claude"
echo '{"env":{"ANTHROPIC_BASE_URL":"https://old.example.invalid","ANTHROPIC_API_KEY":"sk-old"}}' \
  > "$CC_TEST_HOME/.claude/settings.json"

start_server "$PORT"

echo "== 先造一个供应商，导出内容里应该有它"
call '{"cmd":"add_provider","args":{"app":"claude","provider":{"id":"io-test","name":"IO Test","settingsConfig":{"env":{"ANTHROPIC_BASE_URL":"https://io.example.com","ANTHROPIC_API_KEY":"sk-io"}},"category":"custom"}}}' >/dev/null

echo
echo "== 导出（前端 shim 的流程：先分配路径，再导出，最后下载）"
EXPORT_PATH=$(call '{"cmd":"web_allocate_export_path","args":{"defaultName":"cc-switch-export-e2e.sql"}}' \
  | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"])')
check "分配到服务端路径" "True" "$([ -n "$EXPORT_PATH" ] && echo True || echo False)"
call "{\"cmd\":\"export_config_to_file\",\"args\":{\"filePath\":\"$EXPORT_PATH\"}}" >/dev/null
check "导出文件已生成且非空" "True" \
  "$([ -s "$EXPORT_PATH" ] && echo True || echo False)"
check "导出内容包含该供应商" "1" "$(grep -c 'io-test' "$EXPORT_PATH")"

echo
echo "== 下载"
ENCODED=$(python3 -c 'import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))' "$EXPORT_PATH")
check "下载 HTTP 200" "200" \
  "$(curl -s -o "$CC_TEST_HOME/downloaded.sql" -w '%{http_code}' -H "$CC_COOKIE" "$CC_BASE/api/download?path=$ENCODED")"
check "下载内容与导出文件一致" "True" \
  "$(cmp -s "$EXPORT_PATH" "$CC_TEST_HOME/downloaded.sql" && echo True || echo False)"

echo
echo "== 目录越权防护"
check "/etc/passwd 被拒" "403" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "$CC_COOKIE" "$CC_BASE/api/download?path=/etc/passwd")"
check "穿越路径被拒" "403" \
  "$(curl -s -o /dev/null -w '%{http_code}' -H "$CC_COOKIE" "$CC_BASE/api/download?path=$CC_TEST_HOME/../../etc/hosts")"

echo
echo "== 上传 + 恢复"
UPLOAD_PATH=$(curl -s -X POST -H "$CC_COOKIE" --data-binary "@$CC_TEST_HOME/downloaded.sql" \
  "$CC_BASE/api/upload?name=restored.sql" | python3 -c 'import json,sys;print(json.load(sys.stdin)["path"])')
check "上传得到服务端路径" "True" "$([ -n "$UPLOAD_PATH" ] && echo True || echo False)"
call '{"cmd":"delete_provider","args":{"app":"claude","id":"io-test"}}' >/dev/null
check "删除后列表里没有它" "0" "$(call '{"cmd":"get_providers","args":{"app":"claude"}}' | grep -c 'io-test')"
IMPORT_OK=$(call "{\"cmd\":\"import_config_from_file\",\"args\":{\"filePath\":\"$UPLOAD_PATH\"}}" \
  | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"]["success"])')
check "导入成功" "True" "$IMPORT_OK"
check "恢复后供应商回来了" "1" "$(call '{"cmd":"get_providers","args":{"app":"claude"}}' | grep -c 'io-test')"

echo
echo "== 备份"
check "create_db_backup 返回文件名" "True" \
  "$(call '{"cmd":"create_db_backup","args":{}}' | python3 -c 'import json,sys;print(str(json.load(sys.stdin)["data"]).endswith(".db"))')"
check "list_db_backups 非空" "True" \
  "$(call '{"cmd":"list_db_backups","args":{}}' | python3 -c 'import json,sys;print(len(json.load(sys.stdin)["data"])>0)')"

summary
