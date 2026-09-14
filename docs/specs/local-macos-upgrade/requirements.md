# 需求文档：local-macos-upgrade

## 功能概述

当前本地源码升级需要人工执行构建、签名、备份、替换、重启和服务恢复，步骤分散且容易遗漏。更新来源固定为当前本地 Git 工作树，不依赖 GitHub Releases 或远程更新服务。

## 需求列表

### FR-1：单命令升级

WHEN 维护者执行 `npm run install:local` THEN 系统 SHALL 在 macOS 上从当前已提交源码构建并安装 `/Applications/Coding Tools MCP.app`。

### FR-2：升级前门禁

WHEN 升级开始 THEN 系统 SHALL 验证五处应用版本一致、已跟踪文件无未提交变更，并依次通过 Svelte 检查、Cargo 检查和 Tauri App 构建。WHEN 使用 `--dry-run` THEN 系统 SHALL 只验证环境、版本和计划，不停止或替换应用。

### FR-3：可恢复替换

WHEN 构建通过 THEN 系统 SHALL ad-hoc 签名并验证新 App，备份当前 App 与配置，使用 `/Applications` 内的暂存路径替换。WHEN 需要停止旧应用 THEN 系统 SHALL 只终止安装路径对应的精确可执行文件；WHEN 替换失败 THEN 系统 SHALL 恢复旧 App。

### FR-4：服务状态接力

WHEN 升级前检测服务 THEN 系统 SHALL 从配置中的端口识别当前 App 进程实际监听的 MCP 与 Actions 服务。接力文件 SHALL 只保存目标版本、工作区 ID 和服务种类，不保存密钥、URL 令牌或完整配置。WHEN 新版本读取匹配版本的接力文件 THEN 系统 SHALL 先删除它，再恢复相同服务。

### FR-5：安装后验收

WHEN 替换完成 THEN 系统 SHALL 重新打开 App，并验证安装版本、签名和配置哈希。IF 升级前存在运行服务 THEN 系统 SHALL 等待新版本写出恢复结果并确认相应端口重新监听。系统 SHALL 在终端显示验收结果与备份位置。

## 非功能需求

- 仅支持 macOS；其他平台快速失败。
- 默认不使用 `sudo`，权限不足时清楚报错。
- 所有子命令使用参数数组调用，不拼接用户输入为 shell 命令。
- 临时目录和旧 App 路径必须唯一，避免覆盖既有备份。
- 现有未跟踪文件不影响升级，也不得被加入、删除或复制到安装包之外。

## 验收标准

1. `npm run install:local -- --dry-run` 不改变 `/Applications` 和应用数据。
2. 版本不一致或存在已跟踪改动时，命令在构建前失败。
3. 纯函数测试覆盖版本读取、运行服务筛选与参数解析。
4. 新版本启动时只处理目标版本相同的接力文件，并在处理前删除接力文件。
5. 一次真实本地升级后，版本、签名、配置哈希和原运行服务均通过验证。

## 依赖关系

- Node.js 与 npm：运行编排器、前端检查和 Tauri 构建。
- Rust/Cargo 与现有 Tauri CLI：执行后端检查和生成 App bundle。
- macOS 自带的 `codesign`、`ditto`、`open`、`lsof`、`ps`：完成签名、复制、启动和精确状态识别。
- 现有 `AppState`、工作区配置与运行时启动链路：恢复 MCP、Actions 和其关联隧道。
