# 任务
## 交付物清单
生产：pdf.rs 新建，mod.rs/listener.rs/openapi.rs 小范围修改。测试：pdf_tests.rs 新建。文档：docs/actions-pdf.md 与规格。
## 任务列表
### 阶段 1
- [x] 1.1 增加受鉴权 PDF 返回和短时缓存（FR-1、FR-2、FR-3；设计：文件接口、下载生命周期）。证据块：listener.rs serve 鉴权路由；涉及 pdf.rs 约 260 行，listener.rs 约 10 行，mod.rs 1 行。
## 阶段 2
- [x] 2.1 添加专用 OpenAPI 与使用说明（FR-4；设计：OpenAPI 与兼容）。证据块：openapi.rs build_openapi 只返回 ToolExecutionResponse；涉及 openapi.rs 约 45 行、docs/actions-pdf.md 约 70 行。
## 阶段 3
- [x] 3.1 使用合成 PDF 核验全部验收标准（FR-1 至 FR-4；设计：测试及风险）。证据块：现有 listener.rs 仅端口冲突单测；新增 pdf_tests.rs 约 200 行，cargo test actions::。
## 需求覆盖矩阵
| 需求 | 任务 | 状态 |
|---|---|---|
| FR-1、FR-2、FR-3 | 1.1、3.1 | 完成 |
| FR-4 | 2.1、3.1 | 完成 |
## 检查点
规格检查通过再实现；受影响测试通过后审查；最后记录本地验证与 ChatGPT 端未验证边界。

## 文件变更清单
pdf.rs 新建约 260 行；pdf_tests.rs 新建约 200 行；mod.rs 新增模块声明；listener.rs 新增路由合并；openapi.rs 新增路径；docs/actions-pdf.md 新增说明。
