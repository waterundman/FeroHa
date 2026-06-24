# 编排中枢真实数据流设计

**日期：** 2026-06-24

**状态：** 待书面确认

## 目标

将现有编排中枢从“可展示的运行部件”推进为真实可恢复的数据闭环：

```text
人类目标
  -> Orchestrator 确定性工作流
  -> AI Manager 调度 Research AgentTask
  -> Dream Working Memory 原始产物
  -> Scientist + 契约验证
  -> Dream Semantic Memory 已验证知识
  -> Workflow succeeded
```

首版不依赖模型生成 Workflow IR。Orchestrator 使用确定性模板生成一个 Research 工作流，模型仅执行研究内容。

## 系统不变量

### 人类面与 AI 面隔离

1. 人类笔记目录只保存人类创建、编辑和明确输入的内容。
2. AI、Orchestrator、Scientist、Dream 和 Subagent 不得写入人类笔记正文。
3. 所有机器生成正文、研究报告、证据包、验证报告和派生知识只能写入 `.dualtrack/`。
4. Bridge 只能提供观察、控制、反馈和引用，不得把 AI 正文合并到人类笔记。
5. 用户手动复制或改写 AI 内容属于人类输入，不属于系统自动写入。

### Dream 三区生命周期

- `working`：任务过程、原始结果、检索证据、trace、失败记录。
- `semantic`：经过 Scientist 和契约验证的结构化知识。
- `long_term`：仅由 Dream 巩固、去重和稳定性评估产生。

内容只能按 `working -> semantic -> long_term` 晋升。验证失败的内容保留在 `working`，不得进入 `semantic`。普通 Workflow Runtime 不得直接写入 `long_term`。

### 运行状态与知识内容隔离

- `.harness/runs/<run_id>/runtime.json` 保存状态机、步骤、任务 ID、错误码、产物引用和 hash。
- `.harness/runs/<run_id>/events.jsonl` 保存审计事件。
- `.harness` 不保存 AI 正文。
- AI 正文继续使用现有 `.dualtrack/research/...` Working Memory 路径，不新增平行产物仓库。

## 现有架构复用

| 职责 | 复用模块 | 本轮处理 |
|---|---|---|
| Workflow 状态与恢复 | `harness/workflow_runtime.rs` | 扩展产物引用和验证状态 |
| 启动、恢复、幂等调度 | `ai/workflow_runtime_service.rs` | 增加模板入口和即时完成回写 |
| Workflow 到任务转换 | `ai/workflow_task_adapter.rs` | 保持 Research-only |
| 任务管理 | `ai/manager.rs`、`ai/agent_scheduler.rs` | 不建第二套 Scheduler |
| 真实任务执行 | `ai/commands.rs::start_task_worker` | 接入 Runtime 生命周期回调 |
| Working Memory | `ai/research_trace.rs`、`ai/research_graph.rs` | 直接作为 Workflow 原始产物 |
| Scientist | `harness/scientist.rs` | 复用 claims、sources、evidence chain |
| 一致性验证 | `harness/proposition_kernel.rs` | 复用验证结果 |
| Dream 分区 | `ai/dream_memory.rs` | 增加安全的 Semantic 晋升 helper |
| 运行审计 | `WorkflowRuntimeEventStore` | 继续使用同一 ledger |
| 前端状态 | `hooks/useAppStore.ts` | 扩展现有 slice，不新增 Store |
| 编排主视图 | `OrchestratorWorkflowView.tsx` | 增加目标入口和运行详情 |
| 全局状态条 | `OrchestratorPanel.tsx` | 保持宏观摘要，不重复主视图 |
| AI 文件浏览 | `VaultBrowser.tsx` | 继续显示 Dream 三区并刷新产物 |

## 明确不新增

- 不新增 Workflow 数据库。
- 不新增第二套 Scheduler 或 Worker。
- 不新增 Workflow 专属正文目录。
- 不新增独立的全局 Contract Verifier 服务。
- 不新增第二个前端状态容器。
- 不新增与 `OrchestratorWorkflowView` 竞争的编排页面。
- 不让 JSON-LD、MDT 或 Dream Engine 代替 Workflow Runtime。

## 工作流模板

后端增加一个纯构建函数，根据目标和验收条件生成：

- 一个 `GoalContract`；
- 一个只包含 Research step 的 `WorkflowIr`；
- 一个只注册 `research_subagent` 的 `AgentRegistry`；
- 安全、确定性的 `workflow_id` 和 `run_id`。

Research step 使用：

- `kind = research`
- `mode = read_only`
- `status = ready`
- `max_parallel_steps = 1`
- 用户目标作为研究问题
- 用户验收条件作为 `acceptance_criteria`
- 只读研究 Sandbox

验证不是第二个 AgentTask。它是 Research 完成后的同步完成门槛，避免引入重复执行层和依赖死锁。

## 真实数据流

### 创建与启动

1. 用户在现有编排页输入目标和验收条件。
2. 前端调用 `create_and_start_workflow`。
3. 后端模板构建器生成 Goal、Workflow 和 Registry。
4. `WorkflowRuntimeService::start` 校验并写入 `runtime.json`。
5. Runtime 使用现有 `WorkflowTaskAdapter` 创建确定性 Research task。
6. `AiManagerService` 提交并以 `checked_by = orchestrator` 批准只读任务。
7. Worker 被唤醒，事件写入现有 run ledger。

### 执行与 Working Memory

1. Worker 使用现有 LLM、Subagent、向量检索、网络检索、研究图和 trace 链路。
2. 原始结果继续写入：

```text
.dualtrack/research/results/<task_id>/result.md
.dualtrack/research/results/<task_id>/context.json
.dualtrack/research/paths/<task_id>/path_log.jsonl
.dualtrack/research/paths/<task_id>/cot_log.md
.dualtrack/research/graphs/<task_id>/
```

3. 这些现有路径已由 `dream_memory` 分类为 Working Memory。
4. Runtime 只记录产物 ID、`.dualtrack/...` 相对路径、hash、MIME 类型和 producer step。
5. Runtime 的 `dispatch.detail` 只保存短状态或错误摘要，不再保存完整模型输出。

### 自动契约验证

Worker 完成 Workflow task 后，Runtime 立即读取现有任务状态和 Working Memory 产物，并执行确定性验证：

1. Research 结果正文非空。
2. `result.md` 存在且位于 Dream Working Memory。
3. task trace 存在。
4. `TaskContext.retrieval_evidence` 至少包含一条证据。
5. Scientist 能提取至少一个 claim。
6. Scientist evidence chain 至少包含一项带 source ref 的证据。
7. PropositionKernel 若产生验证结果，则不得存在 error violation。
8. 每条验收条件都生成明确的验证记录。

验证输出复用现有 `StepReport` 和 `VerificationFinding`：

- 通过：Research step 由 `reported` 转为 `verified`，run 转为 `succeeded`。
- 失败：step 转为 `failed`，run 转为 `failed`，记录稳定 reason code。
- 无法验证：保留 Working 产物，使用 `cannot_verify` finding，不晋升 Semantic。

### Semantic 晋升

验证通过后，后端从 Scientist 结果生成结构化、带 provenance 的 Semantic Memory 文档：

```text
.dualtrack/memory/semantic/workflows/<workflow_id>/<run_id>.md
```

文档至少包含：

- workflow ID、run ID、task ID；
- 原始 Working Memory 产物引用和 hash；
- claims；
- evidence chain；
- confidence；
- 验收条件与验证结果；
- 生成时间和验证内核名称。

Semantic 写入必须通过 `dream_memory` 内部 helper，目标路径强制位于 canonical semantic root。Runtime 不直接拼接任意文件路径。

## 运行终态

成功条件：

- Research step 已执行；
- Working Memory 产物已落盘；
- 契约验证通过；
- Semantic Memory 产物已成功写入；
- run 的所有步骤均为 `verified` 或 `skipped`。

只有满足全部条件时：

```text
workflow.status = completed
run.status = succeeded
run.ended_at = <timestamp>
```

Semantic 写入失败时不得假装成功。Working Memory 保留，run 标记失败并记录 `semantic_promotion_failed`。

## API 与状态契约

新增聚焦的 `ai/workflow_commands.rs`，避免继续扩大 `ai/commands.rs`：

```text
create_and_start_workflow(goal_text, acceptance_criteria)
get_workflow_run(run_id)
list_workflow_runs()
```

`create_and_start_workflow` 返回 `WorkflowRuntimeBundle`。`get` 和 `list` 从现有 Runtime Store 读取真实磁盘状态。

前端 TypeScript 镜像以下已有 Rust 类型：

- `GoalContract`
- `WorkflowIr`
- `WorkflowRunState`
- `WorkflowDispatchRecord`
- `ArtifactRef`
- `StepReport`
- `VerificationFinding`
- `WorkflowRuntimeBundle`

不使用松散的 `Record<string, unknown>` 作为主要 Workflow 运行契约。

## Worker 接线

现有 Worker 在以下时点调用 Runtime：

- dequeue 后：同步 `running`；
-成功并写完 Working Memory 后：同步 `reported`、验证、Semantic 晋升和终态；
- 执行失败后：同步 `failed`；
- join failure 后：也必须同步 `failed`，不能只发前端事件。

普通非 Workflow task 通过 `workflow_task_context` 检测后直接跳过 Runtime 回调。

Runtime 回调从当前 vault path 构建现有 `WorkflowRuntimeService`，不把 Service 放进新的全局状态。

## 人类笔记写入边界修正

### Diff 与 Ghost

现有 `accept_diff` 会将 Ghost 的 AI 文本写入人类笔记，违反系统不变量。本轮改为：

- “接受”表示接受为 AI 反馈信号；
- 更新 Ghost block 状态和 Trust/Bridge 状态；
- 不调用 `VaultManager::write_note`；
- 不触发人类笔记 `file-changed`；
- UI 文案从“应用到笔记”调整为“采纳反馈”。

AI 建议仍可在 DiffView 查看，人类可以自行在编辑器中书写。

### 人类笔记命令

`save_note`、`create_note`、`delete_note`、`rename_note` 和 `create_folder` 属于人类面写入命令，必须拒绝 `.dualtrack`、`.harness`、隐藏路径和下划线私有路径。

AI 产物不复用这些 IPC 命令，统一由后端内部 Dream/Research writer 写入 `.dualtrack`。

## 前端设计

### OrchestratorWorkflowView

现有只读视图升级为主操作面：

1. 顶部目标输入：
   - 目标文本域；
   - 验收条件列表；
   - 启动按钮；
   - 提交中、错误和 vault 未打开状态。
2. 当前运行摘要：
   - run 状态；
   - Research step 状态；
   - Working 产物数量；
   - 验证结果；
   - Semantic 晋升状态。
3. 步骤列表：
   - queued、running、reported、verified、failed；
   - task ID；
   - 错误 reason code；
   - Working/Semantic 产物链接。
4. 事件时间线：
   - 复用现有 `workflowEventLabel/detail`；
   - 不显示完整原始 JSON 作为主信息。
5. 空状态：
   - 明确说明“输入目标后由编排中枢启动只读研究工作流”。

### OrchestratorPanel

保留底部宏观状态条，只显示：

- 当前 run 状态；
- active task 数；
- verification failure 数；
- replan 数；
- 最近一条异常。

详细步骤、产物和事件只在主编排页展示，避免两个组件重复承载完整信息。

### Dream 文件显示

Workflow 完成或产物变化后刷新现有 `list_ai_workspace_files`：

- Working Memory 立即出现原始研究文件；
- 验证通过后 Semantic Memory 出现晋升文档；
- Long-Term 不因 Workflow 完成自动出现内容。

AI 文件继续只读。人类笔记树不出现 `.dualtrack` 文件。

### DiffView

- “接受”改为“采纳反馈”；
- 显示“不会修改人类笔记正文”；
- 成功提示改为已记录反馈；
- 不再展示已经合并到笔记的暗示。

## 错误处理

稳定 reason code 至少包括：

- `goal_required`
- `acceptance_criteria_required`
- `workflow_runtime_exists`
- `task_execution_failed`
- `task_join_failed`
- `working_artifact_missing`
- `empty_research_result`
- `trace_missing`
- `evidence_missing`
- `claims_missing`
- `verification_failed`
- `verification_cannot_verify`
- `semantic_promotion_failed`

所有失败都必须：

- 写入 runtime；
- 写入 event ledger；
- 在前端展示简短说明；
- 保留已有 Working Memory 产物；
- 不写入 Semantic 或 Long-Term。

## 测试策略

### Rust

- 模板生成稳定、可校验且只有一个 Research step。
- Workflow task 使用现有 Scheduler 且只提交一次。
- Worker 完成后立即同步 Runtime，不依赖手动 resume。
- 完整模型输出不进入 `runtime.json` 或 `events.jsonl`。
- Working artifact 路径全部由 `dream_memory` 分类为 Working。
- 验证失败时没有 Semantic 文件。
- 验证通过时生成一个 canonical Semantic 文件并记录 hash。
- 非 Dream 流程不能写入 Long-Term。
- vault 重启恢复不重复任务。
- `accept_diff` 只记录反馈，不修改源笔记。
- 人类笔记命令拒绝 `.dualtrack`、`.harness` 和隐藏路径。

### 前端

- 目标和验收条件提交正确 IPC payload。
- loading、错误、空状态和终态显示正确。
- Working/Semantic 产物引用可见。
- 编排主视图与底部状态条不重复完整详情。
- DiffView 使用“采纳反馈”语义。
- AI 文件只读且仍按 Dream 三区显示。

### 集成验证

在临时 vault 启动一个真实 Research Workflow，确认：

```text
.harness/runs/<run_id>/runtime.json 存在但不含模型正文
.harness/runs/<run_id>/events.jsonl 存在
.dualtrack/research/results/<task_id>/result.md 存在
.dualtrack/research/results/<task_id>/context.json 存在
.dualtrack/memory/semantic/workflows/<workflow_id>/<run_id>.md 存在
run.status == succeeded
人类笔记内容未发生变化
resume 不产生重复 task
```

## 验收标准

1. 用户可以在现有编排页提交目标并启动真实 Workflow。
2. Research 通过现有 AI Manager 和 Worker 执行。
3. 原始 AI 产物只进入 Dream Working Memory。
4. 通过验证的知识只进入 Dream Semantic Memory。
5. Runtime 和 ledger 不保存 AI 正文。
6. Long-Term 只能由 Dream 巩固流程写入。
7. 人类笔记不会被 AI、Bridge、Ghost 或 Diff 自动修改。
8. 刷新或重启后能够恢复真实运行状态且不会重复执行。
9. 前端清晰显示真实步骤、产物、验证和错误状态。
10. 不引入第二套 Scheduler、Store、产物仓库或编排视图。
