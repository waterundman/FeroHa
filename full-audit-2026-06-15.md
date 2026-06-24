---
title: "FeroHa v3.0.0 全量审查报告"
date: "2026-06-15"
version: "3.0.0"
project: "feroha"
tags:
  - artifact/audit
  - version/3.0.0
  - project/feroha
  - confidence/high
confidence: 0.88
upstream:
  - "[[preview]]"
  - "[[spec/v3.0.0/main]]"
downstream:
  - "[[bayesian-plan]]"
---

# FeroHa v3.0.0 全量审查报告

## 审查目标

1. 识别冗余功能
2. 分析功能之间的实际联系
3. 评估架构合理性

---

## 一、项目架构概览

### 1.1 技术栈

| 层级 | 选型 | 版本 |
|------|------|------|
| 宿主框架 | Tauri | 2.0 |
| 核心逻辑 | Rust | 2021 edition |
| UI | React + TypeScript | 18.3 |
| 编辑器 | CodeMirror | 6 |
| 状态管理 | Zustand | 4.5 |
| 向量库 | SQLite (rusqlite) | 0.39 |
| 全文搜索 | Tantivy | 0.22 |
| 图谱可视化 | D3-force | 3.0 |

### 1.2 模块统计

| 位置 | 模块数 | 主要功能 |
|------|--------|----------|
| `src-tauri/src/ai/` | 20 | AI代理、向量、搜索、记忆 |
| `src-tauri/src/harness/` | 10 | 编排、工作流、验证 |
| `src-tauri/src/` (其他) | 8 | 文件系统、图谱、解析、插件 |
| `src/components/` | 43 | UI组件 |
| `src/hooks/` | 3 | 状态管理 |
| `src/lib/` | 8 | IPC、管道、模板 |
| `src/types/` | 5 | 类型定义 |

---

## 二、冗余功能识别

### 2.1 任务管理组件重复 ⚠️ 高优先级

**问题**: 存在3个功能重叠的任务管理组件

| 组件 | 文件 | 功能 | 状态 |
|------|------|------|------|
| `TaskPanel` | `src/components/TaskPanel.tsx` | 基础任务列表 | 333行 |
| `AgentDashboard` | `src/components/AgentDashboard.tsx` | Agent任务仪表盘 | 1388行 |
| `HumanTaskIntake` | `src/components/HumanTaskIntake.tsx` | 人工任务提交 | 467行 |

**重叠分析**:
- `TaskPanel` 和 `AgentDashboard` 都显示任务状态
- `HumanTaskIntake` 是独立的任务提交入口
- 前端状态管理中 `tasks` 和 `orchestratorStatus` 分离

**建议**:
- 合并 `TaskPanel` 到 `AgentDashboard`
- 保留 `HumanTaskIntake` 作为独立提交入口
- 统一任务状态模型

### 2.2 编排面板重复 ⚠️ 中优先级

**问题**: 存在2个编排相关组件

| 组件 | 文件 | 功能 |
|------|------|------|
| `OrchestratorPanel` | `src/components/OrchestratorPanel.tsx` | 编排状态面板 (862行) |
| `OrchestratorWorkflowView` | `src/components/OrchestratorWorkflowView.tsx` | 工作流视图 |

**分析**:
- `OrchestratorPanel` 显示编排中枢状态、agent状态、诊断信息
- `OrchestratorWorkflowView` 显示工作流执行状态
- 两者都依赖 `orchestratorStatus` 状态

**建议**:
- 合并为统一的编排视图
- 或明确分工：`OrchestratorPanel` 负责宏观状态，`OrchestratorWorkflowView` 负责工作流详情

### 2.3 指令卡组件重复 ⚠️ 低优先级

**问题**: 存在3个指令卡相关组件

| 组件 | 文件 | 功能 |
|------|------|------|
| `CommandCard` | `src/components/CommandCard.tsx` | 单个指令卡 |
| `CommandCardLibrary` | `src/components/CommandCardLibrary.tsx` | 指令卡库 (1945行) |
| `CommandCardPanel` | `src/components/CommandCardPanel.tsx` | 指令卡面板 |

**分析**:
- `CommandCardLibrary` 已包含完整的CRUD功能
- `CommandCardPanel` 可能是旧版本残留

**建议**:
- 检查 `CommandCardPanel` 是否仍被使用
- 如果仅用于展示，可合并到 `CommandCardLibrary`

### 2.4 搜索系统三层重叠 ✅ 设计合理

**分析**: 搜索系统包含三层，但这是有意设计

| 层级 | 文件 | 用途 |
|------|------|------|
| 全文搜索 | `search_engine.rs` (Tantivy) | 关键词搜索 |
| 向量搜索 | `vectordb.rs` | 语义搜索 |
| 混合检索 | `rag.rs` | 融合搜索 |

**结论**: 这是RAG架构的标准设计，不是冗余

### 2.5 图谱系统双层 ✅ 设计合理

**分析**: 图谱系统包含两层

| 层级 | 文件 | 用途 |
|------|------|------|
| 链接图 | `link_graph.rs` | wikilink关系图 |
| 研究图 | `research_graph.rs` | 研究过程图 |

**结论**: 两者服务不同场景，不是冗余

---

## 三、功能联系分析

### 3.1 AI子系统内部依赖链

```
用户提交任务
    ↓
HumanTaskIntake → dispatch_agent_task (IPC)
    ↓
agent_scheduler.rs (任务状态机)
    ↓
task_intent.rs (意图分类) → sandbox.rs (权限策略)
    ↓
subagent.rs (多源检索)
    ↓
┌───────────┼───────────┐
↓           ↓           ↓
vectordb.rs rag.rs   search_engine.rs
(向量)      (混合)    (全文)
↓           ↓           ↓
└───────────┼───────────┘
            ↓
    research_graph.rs (研究图谱)
            ↓
    scientist.rs (知识提炼)
            ↓
    proposition_kernel.rs (一致性验证)
            ↓
    orchestrator.rs (编排审计)
            ↓
    workflow.rs (工作流管理)
            ↓
    bridge/proposal.rs (人工审批)
            ↓
    diff/ghost_store.rs (差异存储)
            ↓
    前端 DiffView (展示)
```

### 3.2 记忆系统依赖链

```
文件变更事件
    ↓
fs/watcher.rs (文件监听)
    ↓
sync_engine.rs (同步引擎)
    ↓
chunker.rs (文本分块) + embedding.rs (向量化)
    ↓
vectordb.rs (向量存储)
    ↓
dream_engine.rs (记忆巩固)
    ↓
┌───────────┼───────────┐
↓           ↓           ↓
NREM        REM        Insight
(强化)      (桥接)     (发现)
↓           ↓           ↓
└───────────┼───────────┘
            ↓
    graph/link_graph.rs (图谱更新)
            ↓
    前端 GraphView (可视化)
```

### 3.3 前端-后端通信链

```
前端组件
    ↓
hooks/useAppStore.ts (状态管理)
    ↓
lib/ipc.ts (IPC封装)
    ↓
@tauri-apps/api/core (Tauri IPC)
    ↓
src-tauri/src/ai/commands.rs (命令处理)
    ↓
各业务模块
```

### 3.4 模式切换依赖

```
AppMode: "human" | "ai"
    ↓
resolveActivePanelForMode()
    ↓
┌───────────────┼───────────────┐
↓                               ↓
humanPanels                     aiPanels
- editor                        - editor
- task-intake                   - graph
- inspiration                   - tasks
- bridge                        - cards
- diff                          - pipeline
- settings                      - plugins
                                - settings
```

---

## 四、架构问题识别

### 4.1 命令处理中心化 ⚠️

**问题**: `commands.rs` 文件过大 (4208行)

**影响**:
- 难以维护
- 职责不清
- 测试困难

**建议**:
- 按功能域拆分：ai_commands.rs, fs_commands.rs, graph_commands.rs
- 使用命令模式重构

### 4.2 状态管理分散 ⚠️

**问题**: 前端状态分散在多个位置

| 位置 | 状态 |
|------|------|
| `useAppStore.ts` | 全局应用状态 |
| `commandCardStore.ts` | 指令卡状态 |
| `useSettings.ts` | 设置状态 |
| 组件本地状态 | UI临时状态 |

**建议**:
- 统一状态管理策略
- 考虑使用Zustand的slice模式

### 4.3 类型定义分散 ⚠️

**问题**: TypeScript类型定义分散

| 位置 | 类型 |
|------|------|
| `src/types/` | 5个类型文件 |
| `src/components/` | 组件内联类型 |
| `src/hooks/` | Store类型 |

**建议**:
- 集中管理核心类型
- 组件类型保持内联

### 4.4 测试覆盖不均 ⚠️

**问题**: 测试覆盖不均匀

| 模块 | 测试状态 |
|------|----------|
| 后端Rust | ✅ 单元测试充分 |
| 前端组件 | ✅ 25个测试文件 |
| E2E测试 | ⚠️ 仅3个基础测试 |
| 集成测试 | ❌ 缺失 |

**建议**:
- 增加E2E测试覆盖
- 添加集成测试

---

## 五、功能模块依赖矩阵

### 5.1 后端模块依赖

| 模块 | 依赖 | 被依赖 |
|------|------|--------|
| agent_scheduler | subagent, sandbox, task_intent, orchestrator | commands |
| subagent | llm_router, vectordb, research_graph | agent_scheduler |
| vectordb | (无) | subagent, rag, sync_engine, dream_engine |
| dream_engine | vectordb, link_graph | commands |
| orchestrator | agent_scheduler, scientist, workflow | commands |
| scientist | lean_translator, proposition_kernel | orchestrator |
| workflow | orchestrator, sandbox | commands |
| sandbox | (无) | agent_scheduler, workflow, tool_registry |
| tool_registry | sandbox, llm_router | commands |
| rag | vectordb, link_graph | commands |
| search_engine | (无) | commands |
| sync_engine | vectordb, embedding | fs/watcher |
| commands | 所有AI模块 | 前端IPC |

### 5.2 前端组件依赖

| 组件 | 依赖Store | 依赖组件 |
|------|-----------|----------|
| App | useAppStore | 所有面板组件 |
| Editor | useAppStore | TabBar, EditorToolbar |
| GraphView | useAppStore | (无) |
| AgentDashboard | useAppStore | FeroHaIcon |
| OrchestratorPanel | useAppStore | FeroHaIcon |
| HumanTaskIntake | useAppStore | CliBar |
| DiffView | useAppStore | (无) |
| CommandCardLibrary | commandCardStore | FeroHaIcon |

---

## 六、优化建议

### 6.1 高优先级

1. **合并任务管理组件**
   - 将 `TaskPanel` 功能合并到 `AgentDashboard`
   - 统一任务状态模型
   - 减少代码重复 (~300行)

2. **拆分 commands.rs**
   - 按功能域拆分为多个文件
   - 提高可维护性
   - 便于单元测试

### 6.2 中优先级

3. **统一编排面板**
   - 合并 `OrchestratorPanel` 和 `OrchestratorWorkflowView`
   - 提供统一的编排视图

4. **集中类型管理**
   - 将核心类型移到 `src/types/` 目录
   - 使用类型导出统一管理

### 6.3 低优先级

5. **清理未使用组件**
   - 检查 `CommandCardPanel` 使用情况
   - 移除未使用的代码

6. **增加测试覆盖**
   - 补充E2E测试
   - 添加集成测试

---

## 七、结论

### 7.1 架构评价

**优点**:
- 模块化设计清晰
- 双轨架构（人类面/AI面）隔离良好
- AI子系统功能完整
- 记忆系统设计合理

**缺点**:
- 部分组件存在冗余
- 命令处理文件过大
- 状态管理略分散

### 7.2 冗余程度评估

| 类型 | 数量 | 严重程度 |
|------|------|----------|
| 任务管理组件 | 3个 | 高 |
| 编排面板 | 2个 | 中 |
| 指令卡组件 | 3个 | 低 |
| 搜索系统 | 3层 | 无（设计合理） |
| 图谱系统 | 2层 | 无（设计合理） |

### 7.3 功能联系评价

**强联系**:
- AI子系统内部依赖紧密
- 记忆系统各组件协同良好
- 前端-后端通信链清晰

**弱联系**:
- 人类面和AI面之间通过Bridge连接
- 插件系统相对独立

---

## 八、下一步行动

1. [ ] 合并任务管理组件
2. [ ] 拆分 commands.rs
3. [ ] 统一编排面板
4. [ ] 清理未使用代码
5. [ ] 增加测试覆盖

---

*审查完成时间: 2026-06-15*
*审查工具: deep-research + bayesian-planner + obsidian-markdown*
