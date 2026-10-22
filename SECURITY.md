# Security policy / 安全策略

This policy covers the **cc-switch-headless Web server**. Desktop-only issues should be reported to [upstream cc-switch](https://github.com/farion1231/cc-switch/security).

本策略适用于本仓库的无头 Web 服务端。请使用 [本仓库的私密漏洞报告入口](https://github.com/zhangzhiwei-zzw/cc-switch-headless/security/advisories/new)，不要在公开 Issue 中披露未修复漏洞。如果入口不可用，请先开一个不含技术细节或凭据的 Issue 请求私密联系方式。

## Scope / 范围

Web HTTP 接口、静态资源、认证、令牌轮换、上传下载、会话数据、导入备份及部署默认值都属于本仓库的安全范围。浏览器、网络请求和用户上传的内容均视为不可信输入；桌面版对本机 WebView 的信任假设不适用于远程浏览器。

令牌代表服务器管理员权限：持有者能够管理 API Key、修改服务器 CLI 配置、安装 Skills 并执行用户配置的用量脚本。本项目不提供多用户、角色权限或租户隔离。不同信任级别应部署到不同主机名、进程和数据目录；不要把具有管理员令牌的操作描述为受限文件沙箱。

## Deployment / 部署

- 默认仅监听回环地址，远程访问优先使用 SSH 隧道；HTTPS 反代需要设置 Host 白名单和 `--hsts`。
- `--hsts` 同时启用 Secure Cookie；普通本地 HTTP 使用 HttpOnly、SameSite=Strict Cookie。
- 令牌文件使用原子写入，Unix 权限为 0600。手动指定令牌至少 16 字符，只支持字母、数字和 `-_.~`。
- 轮换成功后，旧令牌和已有事件连接失效；保存失败则保留旧令牌。
- `--dist` 仅用于受信任的前端构建；服务端阻止父目录和越界符号链接访问。
- 上传/导出使用独立目录，24 小时过期，容量上限 512 MiB。备份和下载文件可能包含凭据，不要放进公共目录。
- 不要向第三方提供启动日志中的登录链接或完整诊断日志。当前没有完整 CSP，主题初始化脚本仍为内联；不要加载不受信任的前端脚本。

## Reporting / 报告

请提供版本、构建提交、部署方式、触发条件、最小复现和影响范围；使用虚构凭据，隐藏 API Key、访问令牌、会话内容及私人路径。

安全修复会通过 [本仓库 Releases](https://github.com/zhangzhiwei-zzw/cc-switch-headless/releases) 发布。维护重点为最新的 Web 版本；旧版本不承诺持续回补。确认、修复及公开披露时间由维护者根据实际影响协商。
