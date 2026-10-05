#!/usr/bin/env bash
# cc-switch-server 端到端测试的公共部分。
#
# 所有脚本都在**临时 HOME** 里跑，不会碰你真实的 ~/.claude、~/.cc-switch。

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

SERVER_BIN="${CC_SWITCH_SERVER_BIN:-$REPO_ROOT/src-tauri/target/debug/cc-switch-server}"
DIST_DIR="${CC_SWITCH_WEB_DIST:-$REPO_ROOT/dist-web}"

PASS=0
FAIL=0

require_prerequisites() {
  if [ ! -x "$SERVER_BIN" ]; then
    cat >&2 <<EOF
找不到服务端二进制：$SERVER_BIN

先构建：
  cd "$REPO_ROOT/src-tauri"
  cargo build --no-default-features --features server --bin cc-switch-server
（也可用 CC_SWITCH_SERVER_BIN 指定其它路径）
EOF
    exit 1
  fi
  command -v curl >/dev/null || { echo "需要 curl" >&2; exit 1; }
  command -v python3 >/dev/null || { echo "需要 python3" >&2; exit 1; }
}

# 每个用例一个临时 HOME，退出时清理
make_test_home() {
  local prefix="${1:-ccswitch-e2e}"
  CC_TEST_HOME=$(mktemp -d "/tmp/${prefix}-XXXXXX")
  export CC_TEST_HOME
  trap 'cleanup_test_env' EXIT
}

cleanup_test_env() {
  [ -n "${CC_SERVER_PID:-}" ] && kill "$CC_SERVER_PID" 2>/dev/null
  [ -n "${CC_UPSTREAM_PID:-}" ] && kill "$CC_UPSTREAM_PID" 2>/dev/null
  return 0
}

# start_server <端口> [额外参数...]
start_server() {
  local port="$1"; shift
  HOME="$CC_TEST_HOME" "$SERVER_BIN" --port "$port" --dist "$DIST_DIR" "$@" \
    > "$CC_TEST_HOME/server.log" 2>&1 &
  CC_SERVER_PID=$!
  CC_SERVER_PORT="$port"
  CC_BASE="http://127.0.0.1:$port"

  local waited=0
  while [ ! -f "$CC_TEST_HOME/.cc-switch/web-token" ]; do
    sleep 0.5
    waited=$((waited + 1))
    if [ "$waited" -gt 120 ]; then
      echo "!! 服务端起不来，日志：" >&2
      cat "$CC_TEST_HOME/server.log" >&2
      exit 1
    fi
  done

  # 端口真的要能连上：重启场景下令牌文件早就存在，光等它会在数据库初始化完成前
  # 就返回，后面的请求全变成连不上。HTTP 有响应（任何状态码）即视为就绪。
  local waited_port=0
  while ! curl -s -o /dev/null --max-time 1 "$CC_BASE/"; do
    sleep 0.3
    waited_port=$((waited_port + 1))
    if [ "$waited_port" -gt 200 ]; then
      echo "!! 服务端端口没起来，日志：" >&2
      cat "$CC_TEST_HOME/server.log" >&2
      exit 1
    fi
  done

  CC_TOKEN=$(cat "$CC_TEST_HOME/.cc-switch/web-token")
  CC_COOKIE="Cookie: ccswitch_web_token=$CC_TOKEN"
}

# call '<json>' —— 走 /api/invoke
call() {
  curl -s -X POST "$CC_BASE/api/invoke" \
    -H "$CC_COOKIE" -H "Content-Type: application/json" -d "$1"
}

# 断言辅助：check <描述> <期望> <实际>
check() {
  local what="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    printf '  ✓ %s\n' "$what"
    PASS=$((PASS + 1))
  else
    printf '  ✗ %s（期望 %s，实际 %s）\n' "$what" "$expected" "$actual"
    FAIL=$((FAIL + 1))
  fi
}

summary() {
  echo
  if [ "$FAIL" -eq 0 ]; then
    echo "== 全部通过（$PASS 项）"
    exit 0
  fi
  echo "== 失败 $FAIL 项 / 通过 $PASS 项"
  echo "   测试目录保留在 $CC_TEST_HOME（服务端日志：$CC_TEST_HOME/server.log）"
  exit 1
}
