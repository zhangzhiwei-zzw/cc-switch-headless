## Summary / 概述

<!-- Briefly describe what this PR does and why. / 简要描述这个 PR 做了什么以及为什么。 -->

## Related Issue / 关联 Issue

<!-- Link the related issue. Use "Fixes #123" to auto-close it when merged. -->
<!-- 关联相关 Issue。使用 "Fixes #123" 可在合并时自动关闭。 -->

Fixes #

## Screenshots / 截图

<!-- If applicable, add before/after screenshots. / 如有需要，请添加修改前后的截图。 -->

| Before / 修改前 | After / 修改后 |
|-----------------|---------------|
|                 |               |

## Checklist / 检查清单

- [ ] Stated impact on Web/server and desktop builds / 已说明对 Web/server 与桌面构建的影响
- [ ] `pnpm typecheck` passes / 通过 TypeScript 类型检查
- [ ] `pnpm format:check` passes / 通过代码格式检查
- [ ] `cargo clippy -- -D warnings` passes (if Rust code changed) / 通过 Clippy 检查（如修改了 Rust 代码）
- [ ] Updated i18n files if user-facing text changed / 如修改了用户可见文本，已更新国际化文件
- [ ] Command changes: updated capability classification, typed schemas and wrappers where applicable / 命令变化已更新能力分类，关键流程已更新参数/返回值契约和类型化调用
- [ ] `pnpm check:web` passes if commands/capabilities changed / 命令或能力变化时通过 Web 契约及覆盖检查
- [ ] Rust changes: checked server feature Clippy/build as well as desktop compatibility / Rust 改动已检查 server feature 的 Clippy/build 和桌面兼容性
- [ ] Recorded relevant HTTP/browser regression results or why they were not run / 已记录相关 HTTP/浏览器回归结果或未执行原因
- [ ] Updated support/deployment/troubleshooting docs where behavior changed / 行为变化已同步用户支持表、部署或排错文档

## Verification / 验证记录

<!-- List commands actually run, results, and anything not covered. Do not include tokens, API keys or private session content. -->
<!-- 列出实际执行的命令、结果及未覆盖范围；不要提交令牌、API Key 或私人会话内容。 -->
