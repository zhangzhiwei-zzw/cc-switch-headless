#!/usr/bin/env bash
#
# cc-switch-server（Web 模式）安装脚本。
#
#   curl -fsSL https://raw.githubusercontent.com/zhangzhiwei-zzw/cc-switch-headless/main/scripts/install-server.sh | bash
#
# 或者带参数：
#
#   curl -fsSL .../install-server.sh | bash -s -- --service
#
# 它只做四件事：下载预编译二进制 → **校验 sha256**（校验文件缺失即失败，
# 不会静默跳过）→ 装到 ~/.local/bin → 可选写一个 systemd user 服务。
#
# 不喜欢预编译包、想让二进制完全出自本机的话，源码构建只要三步：
#   pnpm install && pnpm build:web
#   cd src-tauri && cargo build --release --no-default-features --features server --bin cc-switch-server
#
set -euo pipefail

REPO="${CC_SWITCH_REPO:-zhangzhiwei-zzw/cc-switch-headless}"
VERSION="${CC_SWITCH_VERSION:-latest}"
INSTALL_DIR="${CC_SWITCH_INSTALL_DIR:-}"
PORT="${CC_SWITCH_PORT:-15800}"
WITH_SERVICE=0
VERIFY=1

die() { printf 'install-server: %s\n' "$1" >&2; exit 1; }
info() { printf '  %s\n' "$1"; }

usage() {
  cat <<'EOF'
用法: install-server.sh [选项]

  --version <tag>     安装指定版本（默认 latest，即最新 Release）
  --dir <目录>        安装目录（默认 /usr/local/bin（root）或 ~/.local/bin）
  --port <端口>       --service 写的服务监听端口（默认 15800）
  --service           写入并启用 systemd **用户**服务（无需 sudo）
  --no-verify         跳过 sha256 校验（不推荐；只在下不动校验文件时用）
  -h, --help          显示本帮助

环境变量：CC_SWITCH_REPO / CC_SWITCH_VERSION / CC_SWITCH_INSTALL_DIR / CC_SWITCH_PORT /
          CC_SWITCH_BASE_URL（整体替换下载源，适合镜像站或内网文件服务）
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="${2:-}"; [ -n "$VERSION" ] || die "--version 需要一个 tag"; shift 2 ;;
    --dir) INSTALL_DIR="${2:-}"; [ -n "$INSTALL_DIR" ] || die "--dir 需要一个目录"; shift 2 ;;
    --port) PORT="${2:-}"; [ -n "$PORT" ] || die "--port 需要一个端口"; shift 2 ;;
    --service) WITH_SERVICE=1; shift ;;
    --no-verify) VERIFY=0; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "未知参数: $1（用 --help 看用法）" ;;
  esac
done

command -v curl >/dev/null || die "需要 curl"
command -v sha256sum >/dev/null || die "需要 sha256sum（coreutils）"

# ── 平台 ──────────────────────────────────────────────────────────
case "$(uname -s)" in
  Linux) ;;
  *) die "只提供 Linux 预编译包；其他平台请从源码构建" ;;
esac

case "$(uname -m)" in
  x86_64|amd64) ARCH=x86_64 ;;
  aarch64|arm64) ARCH=aarch64 ;;
  *) die "暂不提供 $(uname -m) 的预编译包；请从源码构建（见脚本头部注释）" ;;
esac

ASSET="cc-switch-server-linux-${ARCH}"

# CC_SWITCH_BASE_URL 可以整体替换下载源（镜像站、内网文件服务、测试用本地 http）。
if [ -n "${CC_SWITCH_BASE_URL:-}" ]; then
  BASE="${CC_SWITCH_BASE_URL%/}"
elif [ "$VERSION" = "latest" ]; then
  BASE="https://github.com/${REPO}/releases/latest/download"
else
  BASE="https://github.com/${REPO}/releases/download/${VERSION}"
fi

# ── 下载 ──────────────────────────────────────────────────────────
TMPDIR_INSTALL="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_INSTALL"' EXIT

echo "==> 下载 ${ASSET}（${VERSION}）"
if ! curl -fsSL --retry 3 -o "${TMPDIR_INSTALL}/${ASSET}" "${BASE}/${ASSET}"; then
  die "下载失败：${BASE}/${ASSET}
  这个版本可能还没有预编译产物。可以先看看有哪些 Release：
    https://github.com/${REPO}/releases
  或者从源码构建（脚本头部注释里有命令）。"
fi
info "$(du -h "${TMPDIR_INSTALL}/${ASSET}" | cut -f1)"

# ── 校验 ──────────────────────────────────────────────────────────
if [ "$VERIFY" -eq 1 ]; then
  echo "==> 校验 sha256"
  # 这条 curl 不打印自己的错误：取不到校验文件时下面那段提示更清楚
  if curl -fsSL --retry 3 -o "${TMPDIR_INSTALL}/${ASSET}.sha256" "${BASE}/${ASSET}.sha256" 2>/dev/null; then
    ( cd "$TMPDIR_INSTALL" && sha256sum -c "${ASSET}.sha256" >/dev/null ) \
      || die "sha256 校验失败——下载可能被截断或被篡改，已放弃安装"
    info "校验通过"
  else
    # 这里刻意不「跳过校验继续装」：静默降级等于没有校验。
    die "取不到校验文件 ${ASSET}.sha256，无法验证下载内容。
  确认这个 Release 是本仓库发布的，或确认信任来源后加 --no-verify 重跑。"
  fi
else
  echo "==> 已跳过 sha256 校验（--no-verify）"
fi

# ── 安装 ──────────────────────────────────────────────────────────
if [ -z "$INSTALL_DIR" ]; then
  if [ "$(id -u)" -eq 0 ]; then INSTALL_DIR=/usr/local/bin; else INSTALL_DIR="$HOME/.local/bin"; fi
fi

mkdir -p "$INSTALL_DIR"
install -m 755 "${TMPDIR_INSTALL}/${ASSET}" "${INSTALL_DIR}/cc-switch-server"
echo "==> 已安装到 ${INSTALL_DIR}/cc-switch-server"
"${INSTALL_DIR}/cc-switch-server" --help >/dev/null 2>&1 \
  && info "二进制可执行" \
  || die "装好的二进制跑不起来——很可能是系统的 glibc 比产物要求的还旧"

case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *) echo "    提示：${INSTALL_DIR} 不在 PATH 里，用绝对路径或在 shell 配置里加上它" ;;
esac

# ── systemd user 服务 ─────────────────────────────────────────────
if [ "$WITH_SERVICE" -eq 1 ]; then
  command -v systemctl >/dev/null || die "--service 需要 systemd（这台机器上找不到 systemctl）"
  UNIT_DIR="$HOME/.config/systemd/user"
  mkdir -p "$UNIT_DIR"
  cat > "${UNIT_DIR}/cc-switch-server.service" <<EOF
[Unit]
Description=CC Switch web server (provider manager + local routing)
After=network-online.target

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/cc-switch-server --port ${PORT}
WorkingDirectory=%h
Environment=RUST_LOG=info
Restart=on-failure
RestartSec=3
TimeoutStopSec=20
KillSignal=SIGTERM

[Install]
WantedBy=default.target
EOF

  echo "==> 已写入 ${UNIT_DIR}/cc-switch-server.service"
  systemctl --user daemon-reload
  systemctl --user enable --now cc-switch-server
  systemctl --user --no-pager --lines=0 status cc-switch-server || true
  echo "    未登录也保持运行：loginctl enable-linger $USER"
  echo "    看日志：journalctl --user -u cc-switch-server -f"
fi

# ── 下一步 ────────────────────────────────────────────────────────
TOKEN_FILE="$HOME/.cc-switch/web-token"
cat <<EOF

==> 完成。启动：

    ${INSTALL_DIR}/cc-switch-server --port ${PORT}

启动日志里会打印一条带令牌的地址，用它打开浏览器即可：

    http://127.0.0.1:${PORT}/auth?token=<令牌>

令牌也存在 ${TOKEN_FILE}。远程访问推荐走 SSH 端口转发：

    ssh -L ${PORT}:127.0.0.1:${PORT} user@这台服务器

换令牌（不用重启，旧令牌当场失效）：

    curl -X POST http://127.0.0.1:${PORT}/api/rotate-token \\
      -H "Cookie: ccswitch_web_token=\$(cat ${TOKEN_FILE})" \\
      -H 'Content-Type: application/json' -d '{}'
EOF
