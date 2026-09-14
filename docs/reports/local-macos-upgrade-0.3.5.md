# 本地 macOS 一键升级验收（0.3.5）

## 结果

`npm run install:local` 已从当前本地提交构建并替换 `/Applications/Coding Tools MCP.app`。最终安装版本为 0.3.5，ad-hoc 签名通过 `codesign --verify --deep --strict`。

升级前脚本识别到“量化研究”工作区的 MCP 与 Actions 两个服务。新应用启动后，一次性接力结果为 `success: true`；端口 28766 与 8788 均由新应用 PID 监听，本地 MCP 与 Actions OpenAPI 请求均返回 HTTP 200。升级前后配置 SHA-256 一致。

最终备份位于：

`/Users/robot/Library/Application Support/coding-tools-mcp-desktop/data/update-backups/2026-09-14T16-45-52-535Z-to-0.3.5`

## 验证

- Node 本地安装测试：5 项通过。
- Rust 全量测试：207 项通过，0 失败，1 项需要独立浏览器环境的测试保持 ignored。
- Svelte：0 errors，0 warnings；生产构建通过。
- dry-run：确认目标版本与 2 个运行服务，未构建、未停止应用、未修改文件。
- 真实升级：版本、签名、配置、MCP 28766、Actions 8788 全部通过。

## 现场修正

首次安装验证发现恢复模块使用了 Tauri 默认数据目录；已改为项目现有 `Platform::app_config_dir()/data`。第二次验证发现 LaunchServices 环境不包含 `/usr/sbin`；macOS 端口检测已固定调用 `/usr/sbin/lsof`。0.3.5 在正常 LaunchServices 启动环境下完成最终验收。

## 使用方式

```bash
npm run install:local
```

仅做无副作用预检：

```bash
npm run install:local -- --dry-run
```
