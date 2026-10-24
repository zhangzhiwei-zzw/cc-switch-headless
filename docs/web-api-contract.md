# Web 接口契约与命令接入

接口用于已认证的服务器管理员。浏览器通过同源 HTTP 操作服务端配置；桌面版仍使用 Tauri IPC。这里描述现有协议和接入步骤，不承诺未列出的命令已具有运行时类型校验。

## 协议与认证

| 接口 | 用途 | 认证 |
| --- | --- | --- |
| `GET /healthz` | 进程存活 | 不需要令牌 |
| `GET /readyz` | 数据库就绪，未就绪返回 503 | 不需要令牌 |
| `GET /api/env` | 版本、构建提交、配置目录、上游基线 | Cookie 或 Bearer |
| `GET /api/capabilities` | 功能、应用、可调用命令 | Cookie 或 Bearer |
| `GET /api/commands` | HTTP 已实现命令名 | Cookie 或 Bearer |
| `POST /api/invoke` | 命令请求/响应 | Cookie 或 Bearer |
| `GET /api/events` | SSE 事件流 | Cookie 或 Bearer |
| `POST /api/session-transcript` | NDJSON 会话流 | Cookie 或 Bearer |

上传、下载和令牌轮换也受认证保护，见 [部署说明](web-mode-zh.md)。所有接口均受 Host/Origin 检查；健康接口无需令牌不等于可以绕过这些检查。

## 请求与响应

```json
{"cmd":"set_proxy_route","args":{"appType":"claude","providerId":"example"}}
```

请求使用 `cmd` 和对象 `args`，参数名跟随前端 camelCase。不要把 `app`、`appType` 混用。服务端按各命令解析和校验，Rust 字段需要时使用 `#[serde(rename_all = "camelCase")]`。

```json
{"ok":true,"data":{"kind":"route","clientConfigChanged":true}}
```

```json
{"ok":false,"error":"E_NOT_IMPLEMENTED: 此命令未接入 Web","code":"E_NOT_IMPLEMENTED","requestId":"<服务器生成的 UUID>"}
```

目前 `/api/invoke` 的业务错误通常仍返回 HTTP 200，必须检查 `ok`。HTTP 401、400、403、413 等属于认证、中间件或请求解析层面的失败，不应假定响应一定是上述 JSON。`data` 可以是 `null`，例如成功的 `set_opencode_default_model`；没有默认模型的 `get_opencode_default_model` 也返回 `null`，不等于失败。

Web shim 保持现有调用语义：成功 resolve `data`，失败以字符串 reject。不要仅检查 HTTP 200 就显示“操作成功”。写操作超时也不代表后端已经回滚，确认服务器状态后再决定是否重试。

### 错误分类与请求追踪

服务端响应头携带 `X-Request-Id`，由服务端重新生成，不信任调用者提供的编号。`/api/invoke` 业务失败还返回 `code` 和 `requestId`，仍保留 `error` 字符串兼容原有调用。Web 提示会附带编号，可用于日志关联；认证/JSON 解析等中间件错误不保证使用相同 JSON 信封，但仍有响应头编号。API 与认证响应标记 `Cache-Control: no-store`。

| code | 含义 |
| --- | --- |
| `E_ARGUMENT` | 命令参数解析失败 |
| `E_INIT` | 数据库未就绪 |
| `E_NOT_IMPLEMENTED` | 命令未接入 Web |
| `E_BUSY` | 会话读取的两个并发名额已占满 |
| `E_SESSION_LIMIT` | 会话源、输出或单条消息超限 |
| `E_SESSION_READ` | 会话预检或读取失败 |
| `E_INTERNAL` | 后台任务异常；对外隐藏异常细节 |
| `E_COMMAND` | 其他业务失败；结合 error 判断处理方式 |

`E_CONTRACT` 是浏览器发现结构不匹配时产生的本地错误，不是上述服务端分类。会话流尚未开始读取时遇到并发限制返回 HTTP 429；开始传输后读取失败通过 NDJSON `type=error` 报告，HTTP 状态可能已是 200。客户端必须看到 `type=done` 才算传输完整。

HTTP 日志包含编号、已知命令、状态、错误分类及 `elapsed_ms`；不记录参数或正文，未知 URL 使用固定占位路径。流式接口的 elapsed_ms 是生成响应头的时间，不代表整个流完成耗时。会话流内部失败另外记录编号和错误分类。

## 三层契约

1. **命令覆盖与能力**：`src/web-shims/command-contract.json` 分类 Web、浏览器适配和未支持命令，防止新增调用漏接。能力为真只表明所需命令存在，不代表所有参数和行为都已验证。
2. **类型化参数与结果**：`src/lib/api/commandSchemas.ts` 定义关键命令的 Zod schema；API 包装使用 `invokeChecked`。TypeScript 从 schema 推导调用签名，参数名称与返回字段改动会影响调用方的类型检查。
3. **Web 运行时校验**：shim 校验所有命令的响应信封；对 `validatedCommands` 列出的关键命令还校验发出前的参数和成功返回值。请求 schema 严格检查字段，响应对象允许新增字段，不删除返回数据。

当前覆盖 12 个关键命令，完整列表由 [开发者能力表](web-capabilities.md#参数与返回值契约) 生成。其他命令仍依赖 Rust 参数解析和调用方约定；NDJSON 会话流走独立接口，不在这 12 个 `/api/invoke` 契约内。桌面构建使用同一类型签名，但不经过 Web 的运行时校验。

格式不符合时，Web 抛出 `E_CONTRACT`，包含命令、request/response/envelope、字段路径与问题代码；不会把参数值、密钥或响应正文放进这条错误。它用于发现前后端不匹配，不会替代服务端校验，也不会阻止具有管理员令牌的调用者直接请求 HTTP。

## 示例：OpenCode 默认模型

现有命令可作为最小接入示例：

- Rust：`web/routes.rs` 中 `get_opencode_default_model` 调用 `opencode_config::get_default_model`，通过 `deferred` 在线程池执行并序列化结果。
- 分发表：注册同名 handler。
- 能力：`command-contract.json` 的 OpenCode 应用依赖包含该命令。
- schema：请求为严格空对象，结果为 `string | null`。
- API：`providersApi.getOpenCodeDefaultModel` 调用 `invokeChecked("get_opencode_default_model")`，返回 `Promise<string | null>`。

只读 HTTP 调用示例，在运行服务器的机器执行：

```bash
curl -sS http://127.0.0.1:15800/api/invoke \
  -H "Authorization: Bearer $(cat ~/.cc-switch/web-token)" \
  -H 'Content-Type: application/json' \
  -d '{"cmd":"get_opencode_default_model","args":{}}'
```

若配置目录有覆盖，改用真实目录中的令牌文件。不要把这条命令的响应、令牌或含私人路径的诊断直接贴到公开 Issue。

## 新增命令清单

1. 在共享 `services/` 或已有配置模块实现业务，处理幂等、文件/数据库一致性与失败返回。
2. 在 `web/routes.rs` 或 `web/mcp.rs` 等领域文件做参数解析和 service 调用；阻塞文件/数据库操作使用已有 `blocking`/`deferred`。需要数据库的命令通过 `context.require_state()` 读取状态。
3. 在分发表注册 handler；新增桌面命令时也要检查 server feature 是否受桌面 API 影响。
4. 在 `src/lib/api/` 封装前端调用，加入所属 feature/app 的命令依赖；不支持 Web 的命令必须写明原因并隐藏入口。
5. 对关键流程增加 `rpcContracts` 的 `args`、`result`，同时加入 `validatedCommands`；API 包装使用 `invokeChecked`。涉及其他文件的同名直接 invoke 也须迁移，检查器会拒绝绕开类型化包装。
6. 更改可见功能时更新 `app-support.json`，重新生成支持表。
7. 执行类型、格式、命令和文档检查。适用时验证真实 HTTP 及对应浏览器流程，记录实际结果。

```bash
node scripts/web/check-command-coverage.mjs
node scripts/web/check-rpc-contract.mjs
node scripts/web/generate-capabilities.mjs
pnpm typecheck
pnpm format:check
```

`check-rpc-contract.mjs` 检查声明与 schema 是否对应、handler 是否存在、前端是否有类型化包装以及已覆盖命令是否被原始 invoke 绕开；它不证明 Rust 返回值一定符合 schema。实际返回不匹配由 Web 运行时检查报告，服务端业务语义仍需对应的 HTTP/业务回归覆盖。

`checkedInvoke.ts` 是唯一经过审阅的动态调用出口，命令联合类型由 schema 约束；覆盖扫描对此文件显式豁免，其他动态 invoke 仍须处理。不要扩大豁免范围来跳过漏接检查。

## 同步上游时

- 比较命令名之外，还要复查参数的字段名、可空性、返回结果和保存副作用。
- 优先把公共业务放进 service，避免 Web/桌面各保留一份逐渐分叉的实现。
- 遇到 `E_CONTRACT`，先确认前后端构建版本，再对照 Rust 序列化结果修改契约或适配层。不要用 `z.any()` 或断言成 `true` 绕过差异。
- 开发、发布与上游同步流程分别见 [贡献指南](../CONTRIBUTING.md)、[发布说明](README.md#发布)、[同步流程](syncing-upstream-zh.md)。
