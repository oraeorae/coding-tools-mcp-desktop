# 任务清单：本地浏览器只读工具

## 概述
按协议和安全边界实现三个工具。

## 交付物清单（Scope-lock）
桥接模块与独立依赖锁、Rust 模块与接入、Node/Rust 测试、用户指南及规格三件套。实际新建 14 个交付文件，修改 7 个已有文件（4 个 Rust 模块、1 个 Rust 测试、package.json 和 README）；新模块每个控制在 500 行以内，现有大文件仅添加分发和 schema。

## 任务列表
- [x] 1.1 实现受限本地桥接与扩展适配器，所有输入有界（FR-1、FR-2、FR-3、FR-4；设计：技术方案/API 设计）。
  - 证据块：src-tauri/src/tools/mod.rs:1 `pub mod context;`，新增独立 browser 模块；官方 @playwright/mcp index.d.ts 导出 `createConnection(config, contextGetter)`。
  - 涉及文件：bridge/browser/{adapter,core,server}.mjs 各小于 400 行；package 文件与 test 目录。
- [x] 2.1 接入 Rust 工具注册、分发、MCP 图片封装并保持鉴权（FR-1 至 FR-4；设计：API 设计）。
  - 证据块：dispatch.rs:140 `"view_image" => image_tool::view_image(ws, &effective_args)`；workspace.rs `if tool_name == "view_image"`；registry.rs `pub fn input_schema(name: &str)`。
  - 涉及文件：browser.rs 小于 400 行；mod.rs +1；dispatch.rs +4；registry.rs +70；workspace.rs +2。
- [x] 3.1 用合成网页完成真实浏览器验收及失败用例，文档说明安装连接（FR-1 至 FR-4；设计：测试策略）。
  - 证据块：现有 tests/call_tool_security.rs 和 tools/workspace.rs 的 MCP image 输出契约。
  - 涉及文件：browser_tools.rs 小于 300 行、test/*.test.mjs 各小于 300 行、docs/browser-bridge.md 小于 200 行、README 增加入口。

## 检查点
- [x] 规格完成；实现前通过 check_spec。
- [x] 运行时与三工具实现通过测试。
- [x] 真实浏览器验收、回归、审查和指南完成。

## 需求覆盖矩阵
| 需求 | 设计章节 | 任务 |
| --- | --- | --- |
| FR-1 | API 设计 | 1.1,2.1,3.1 |
| FR-2 | API 设计 | 1.1,2.1,3.1 |
| FR-3 | API 设计 | 1.1,2.1,3.1 |
| FR-4 | 数据模型/设计决策 | 1.1,2.1,3.1 |

## 文件变更清单
以上交付物限定于桥接、工具接入、测试和文档，不改鉴权实现，不部署到用户现有服务。

## 自检清单
- [x] 任务有证据、路径、预算与需求回链。
