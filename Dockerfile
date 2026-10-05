# syntax=docker/dockerfile:1
#
# CC Switch Web 模式（cc-switch-server）镜像。
#
#   docker build -t cc-switch-web .
#   docker run -d --name cc-switch-web -p 127.0.0.1:15800:15800 \
#     -v cc-switch-data:/data cc-switch-web
#   docker logs cc-switch-web | grep token      # 取首次访问的令牌
#
# 前端产物被编译进二进制（rust-embed），运行镜像里没有多余文件。
# 构建基座用 Ubuntu 20.04（glibc 2.31）：产出的二进制也能直接在 20.04 这类
# 老系统上跑，不必依赖更新的 glibc。

# ── 1. 构建前端产物 ────────────────────────────────────────────────
FROM node:22-bookworm-slim AS web-builder
WORKDIR /app
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY src ./src
COPY tsconfig.json tsconfig.node.json vite.config.ts ./
COPY tailwind.config.cjs postcss.config.cjs components.json ./
RUN pnpm build:web

# ── 2. 构建服务端（无 Tauri / 无 WebKit 依赖）──────────────────────
FROM ubuntu:20.04 AS server-builder
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential pkg-config ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# 工具链版本由 rust-toolchain.toml 决定，rustup 会在首次 cargo 调用时按需安装
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain none
ENV PATH=/root/.cargo/bin:$PATH

WORKDIR /app
COPY rust-toolchain.toml ./
COPY src-tauri ./src-tauri
# 前端产物要在这里就位：rust-embed 在编译期把 dist-web 读进二进制
COPY --from=web-builder /app/dist-web ./dist-web

WORKDIR /app/src-tauri
RUN cargo build --release --no-default-features --features server --bin cc-switch-server

# ── 3. 运行 ───────────────────────────────────────────────────────
# 发布流水线用 `--target runtime` 只构建到这一步，再从镜像里取出二进制
# 当预编译产物（见 .github/workflows/release-server.yml）。
FROM ubuntu:20.04 AS runtime
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=server-builder \
     /app/src-tauri/target/release/cc-switch-server /usr/local/bin/cc-switch-server

# 配置目录是 $HOME/.cc-switch（数据库、web-token、备份、导出都在这儿）
ENV HOME=/data
VOLUME ["/data"]

# 容器内必须绑 0.0.0.0 才能被端口映射进来；对外只映射到宿主机回环即可
# （Host 校验默认只认 localhost/127.0.0.1，用域名访问要加 --allow-host）
EXPOSE 15800
ENTRYPOINT ["cc-switch-server"]
CMD ["--bind", "0.0.0.0", "--port", "15800"]
