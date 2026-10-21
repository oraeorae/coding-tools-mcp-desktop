# 任务清单：browser_open
## 概述
导航工具端到端交付。
## 交付物清单（Scope-lock）
新建 4 个文档，修改 11 个文件，约 12 处函数与匿名测试。精确路径见 design.md 文件结构。已有大 registry 文件仅增加 schema/条目，不引入大型函数。
## 任务列表
- [x] 1.1 实现新标签导航及失败回滚（FR-1、FR-2；设计：技术方案/API 设计）。
  - 证据块：bridge/browser/adapter.mjs:24 `if (method !== 'browser_list_tabs' && !args.tab_id)`；core.mjs:3 METHODS 仅包含三个工具。
  - 涉及文件：core.mjs +35、adapter.mjs +6、server.mjs +1，各小于 500 行。
- [x] 2.1 接入 Rust 校验、导航错误及 profile 分发（FR-1、FR-2；设计：API 设计/设计决策）。
  - 证据块：dispatch.rs:140 三个 browser 分支共用 browser::call；registry.rs:354 CORE_READ_ONLY_TOOLS。
  - 涉及文件：browser.rs +25、dispatch.rs +1、registry.rs +14，registry 原大文件不拆既有无关代码。
- [x] 3.1 验证打开后读取、失败保留和只读拒绝并更新指南（FR-1、FR-2；设计：测试策略）。
  - 证据块：src-tauri/tests/browser_tools.rs:49 browser_bridge_end_to_end 使用合成 Chrome；bridge/browser/test/browser.test.mjs 真实浏览器测试已有默认页断言。
  - 涉及文件：Node 测试 +45、Rust browser 测试 +40、call_tool_contract.rs +1、指南 +25、README +1、验证报告新建 <100 行。
## 需求覆盖矩阵
| 需求 | 设计 | 任务 |
| --- | --- | --- |
| FR-1 | 技术方案/API 设计 | 1.1,2.1,3.1 |
| FR-2 | 设计决策/API 设计 | 1.1,2.1,3.1 |
## 检查点
- [x] 规格通过后实现。
- [x] 测试通过并完成审查。
## 文件变更清单
严格遵循 design.md 11 修改、4 新文档列表。保留此前未提交的第一版成果。
## 检查清单
- [x] 需求回链、代码证据、行数预算完整。
