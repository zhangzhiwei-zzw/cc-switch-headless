<div align="center">

# cc-switch-headless

### Headless web server for CC Switch

**Manage providers, local routing and sessions for Claude Code / Codex / Gemini CLI and 7 more AI CLIs — from a browser. Same UI as the desktop app, served over HTTP.**

[![Based on cc-switch v4.0.0](https://img.shields.io/badge/based%20on-cc--switch%20v4.0.0-blue)](https://github.com/farion1231/cc-switch)
[![License: MIT](https://img.shields.io/badge/license-MIT-lightgrey.svg)](LICENSE)

[中文](README.md) | English | [Upstream docs (zh)](README_ZH.md) | [Web mode details (zh)](docs/web-mode-zh.md)

</div>

## Why this fork exists

Upstream [cc-switch](https://github.com/farion1231/cc-switch) is a Tauri desktop app that requires
**glibc 2.35+ / WebKitGTK 4.1** (Ubuntu 22.04 and newer). On Ubuntu 20.04 and similar it cannot run —
and cannot even be compiled.

This fork adds a headless server, `cc-switch-server`:

- serves the **same frontend** over HTTP (one source tree; the web build swaps Tauri's IPC layer for HTTP)
- has **no Tauri / WebKit dependency**, so it builds on Ubuntu 20.04
- **embeds the frontend** into the binary — deployment is copying one file
- runs the local proxy for real: protocol conversion and failover happen server-side

The desktop app is unchanged.

## Quick start

**Install a prebuilt binary** (no Rust toolchain needed; built on Ubuntu 20.04, runs on 20.04
through 24.04):

```bash
curl -fsSL https://raw.githubusercontent.com/zhangzhiwei-zzw/cc-switch-headless/main/scripts/install-server.sh | bash
~/.local/bin/cc-switch-server          # add --service for a systemd user unit
```

**Or build from source**:

```bash
# 1. frontend (Node 22+ and pnpm)
pnpm install && pnpm build:web

# 2. server (no webkit / gtk packages required)
cd src-tauri
cargo build --release --no-default-features --features server --bin cc-switch-server

# 3. run — listens on 127.0.0.1:15800 by default
./target/release/cc-switch-server
```

The startup log prints a URL that carries the access token — open it in a browser:

```
http://127.0.0.1:15800/auth?token=<64-char token>
```

It sets a cookie once, after which `http://127.0.0.1:15800/` works directly.
The token is also stored in `~/.cc-switch/web-token`.

## Running it elsewhere

**Remote server** — the server only listens on loopback, so an SSH tunnel is the easiest way in:

```bash
ssh -L 15800:127.0.0.1:15800 user@your-server
```

**Docker**:

```bash
docker compose up -d                      # see docker-compose.yml
docker logs cc-switch-web | grep token    # grab the token
```

**As a service** — the repository ships a systemd unit:

```bash
cp scripts/systemd/cc-switch-server.service ~/.config/systemd/user/
systemctl --user enable --now cc-switch-server
loginctl enable-linger "$USER"            # keep it running while logged out
```

## What you can do

The UI is the desktop one. From a browser you can:

- **Providers** — add / edit / delete / switch / reorder; the editor previews what the config file will look
  like after switching
- **Local routing** — start the proxy (defaults to `127.0.0.1:15721`), point a CLI's config at it, switch
  between direct and routed mode, configure the failover queue
- **Data** — export an SQL backup (downloaded straight to the browser), restore by uploading one, and
  create / restore / rename / delete backups
- **Sessions** — browse the session logs the CLIs on **that machine** left behind: search, read, delete
- **MCP** — CRUD, per-app toggles, import from the apps' own configs, re-sync back to them
- **Skills** — install / update / uninstall, repo management, import unmanaged ones, migrate storage,
  install from a ZIP (browser picks the file → upload → the server unpacks it), backups, skills.sh search
- **Prompts** — the prompt library plus Pi's native prompt files and templates
- **Usage** — summaries, trends, per-provider / per-model breakdowns, request logs, model pricing and
  models.dev sync, session-log sync and Codex usage rebuild, provider balance and quota queries

## Not ported yet

Managed-account login (Copilot / Codex / xAI) and their quota queries, Stack mode, the circuit-breaker
panel, CLI tool version management, and desktop-only actions such as directory pickers, terminal launch
and "open in file manager".

The UI reads `GET /api/capabilities` and **hides what is not there** instead of leading you into dead ends.

## Security

The server can read and write `~/.claude`, `~/.codex` and friends, holds your API keys, and can execute
external commands. Therefore:

- it **binds to `127.0.0.1` only** by default and validates `Host` / `Origin` (anti DNS rebinding)
- `/api/*` requires the token; `--no-token` is refused unless the bind address is loopback
- responses carry `nosniff` / `X-Frame-Options: DENY` / `Referrer-Policy: no-referrer` and friends
- the token can be **rotated without a restart** (the old one stops working immediately):

  ```bash
  curl -X POST http://127.0.0.1:15800/api/rotate-token \
    -H "Cookie: ccswitch_web_token=$(cat ~/.cc-switch/web-token)" \
    -H 'Content-Type: application/json' -d '{}'
  ```

  Pass `{"token":"your-own-long-enough-token"}` to choose the value yourself.
- use an SSH tunnel for remote access; in containers use `--bind 0.0.0.0` but publish the port to the host
  loopback only; behind a TLS reverse proxy add `--hsts`

Parameters, systemd, Docker and the implementation notes are documented in
[docs/web-mode-zh.md](docs/web-mode-zh.md) (Chinese).

## Credits

Fork of [farion1231/cc-switch](https://github.com/farion1231/cc-switch) by Jason Young, MIT licensed.
**Upstream features and their bugs belong upstream**; the full Chinese documentation for the upstream app is
in [README_ZH.md](README_ZH.md).
