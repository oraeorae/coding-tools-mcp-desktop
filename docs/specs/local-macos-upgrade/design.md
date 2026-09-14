# 设计文档：local-macos-upgrade

## 概述

`scripts/install-local-macos.mjs` 作为唯一升级编排入口，由 `package.json` 暴露为 `install:local`。Node 脚本负责前置门禁、构建、签名、进程识别、备份、替换、启动与验收。应用侧新增 `local_update` 模块，只负责消费一次性服务接力计划。

对应需求：FR-1、FR-2、FR-3、FR-4、FR-5。

## 技术方案

## 升级流程

1. 解析 `--dry-run`，确认 macOS、仓库根目录和必要命令。
2. 从 package、Cargo 与 Tauri 五处读取版本并确认一致；确认已跟踪文件干净。
3. 读取应用配置，并用 `lsof` 判断配置端口是否由当前安装 App 的 PID 监听。
4. dry-run 到此结束；正式模式执行 Svelte/Cargo 检查并构建 App。
5. ad-hoc 签名并验证构建产物，复制到 `/Applications` 下唯一暂存路径。
6. 将配置和旧 App复制到带时间戳的备份目录；写入不含密钥的接力计划。
7. SIGTERM 精确停止旧 App，暂时重命名旧 App，原子启用新 App；失败即恢复。
8. 打开新 App，校验 Info.plist 版本、codesign 和配置 SHA-256。
9. 新 App 读取接力计划，删除计划后调用现有服务启动函数，并原子写入恢复结果；脚本确认结果与监听端口。

## 数据契约

接力计划位于应用数据目录 `local-update-restore.json`：

```json
{
  "targetVersion": "0.3.5",
  "services": [
    { "workspaceId": "opaque-id", "mcp": true, "actions": true }
  ]
}
```

结果位于同目录 `local-update-restore-result.json`，包含目标版本、每项服务的成功或错误文本，不包含认证数据。

## 安全与回滚

- 进程匹配使用完整可执行文件路径，不使用 `pkill` 或名称模糊匹配。
- 监听状态必须同时满足端口 PID 属于精确安装 App 进程。
- 新旧 App 切换发生在 `/Applications` 同一目录；旧 App 在新 App 就位前保留为唯一临时路径。
- 切换失败恢复旧路径；新 App 已验证后再归档临时旧 App。
- 配置只读并备份，脚本不修改其中任何密钥或工作区设置。

## 测试

- Node 单测测试参数、版本一致性和服务筛选；所有 OS/进程依赖通过注入函数隔离。
- Rust 单测测试计划版本过滤、一次性消费和结果序列化的纯逻辑。
- dry-run 验证当前仓库真实版本和环境，不构建、不停止应用。
- 最终使用命令完成一次 0.3.5 本地安装并核对服务恢复。

## 文件结构

```text
scripts/
├── install-local-macos.mjs
└── install-local-macos.test.mjs
src-tauri/src/
└── local_update.rs
docs/specs/local-macos-upgrade/
├── requirements.md
├── design.md
└── tasks.md
```
