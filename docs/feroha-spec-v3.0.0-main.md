---
title: "SPEC v3.0.0"
date: "2026-05-29"
version: "3.0.0"
project: "feroha"
tags:
  - artifact/spec
  - version/3.0.0
  - project/feroha
  - architecture/memory
  - confidence/high
confidence: 0.86
upstream:
  - "[[preview]]"
  - "[[开发仪表盘]]"
  - "[[spec/v2.14.3/main]]"
  - "[[spec/v2.14.3/addendum]]"
  - "[[spec/v2.14.5/main]]"
  - "[[MDT Markdown Tree 最终方案报告]]"
downstream:
  - "[[bayesian-plan]]"
---

# SPEC - FeroHa v3.0.0

## 迭代主题: MDT 记忆架构 + Dream Graph + Sandboxed Orchestrator

### 背景

v2.14.9 已经把 FeroHa 的 AI 面关键设施接到了可运行状态：Bridge Proposal、TaskScheduler、ToolRegistry、Scientist、DreamEngine、Subagent、GraphView、TrustScore 与 OutputHook 都已经形成基本闭环。当前瓶颈不再是“有没有模块”，而是“这些模块是否共享同一个记忆结构协议”。

v3.0.0 因此定义为一次架构大版本：把 AI 面从一组可用工具升级为一个可解释、可审计、可分层展开的记忆系统。MDT 负责文件与索引协议，Dream 负责记忆分块与再组织，GraphView 负责可视化表达，Orchestrator 负责调度而不是直接写入，Bridge 负责人类最终批准。

### 设计原则

1. **人类面仍拥有最终写权**：AI 面可以读取、索引、研究、生成提案，但默认不直接覆写源笔记。
2. **MDT 是格式族，不是新 Markdown 语法**：正文保持 CommonMark，结构信息进入 YAML front matter、manifest、edges 与 indexes。
3. **压缩/解压缩是分层读取，不是摘要还原**：Reader 通过 L0-L3 控制读取量和上下文展开，不能让 AI 根据摘要重造未读取原文。
4. **Dream 连接必须进入图谱协议**：GraphView 的线型、颜色、透明度和节点状态应来自真实的记忆区块与连接类型。
5. **Orchestrator 降权为编排者**：它只计划、分派、审计、汇总，不持有直接写源文件的工具。
6. **Lean 核心重新定位**：当前核心保留为命题图一致性检查器，不作为真值证明器。

---

## 当前基线审计

| 子系统 | 当前状态 | v3.0.0 需要补齐 |
|---|---|---|
| GraphView | Canvas + d3-force，可显示 wikilink 图和 focus activation | GraphEdge 类型、Dream 分块元数据、线型/图例/过滤器 |
| LinkGraph | `GraphEdge { from, to }`，无关系类型 | `edge_type/origin/confidence/memory_region` |
| Parser | frontmatter 只有 `title/tags/aliases` | MDT frontmatter: `id/tree/area/importance/summary/links/storage/content_hash` |
| DreamEngine | 已有 NREM/REM/Insight 与 ConnectionType | 将 Dream connection 和 community 输出到 Graph/MDT index |
| AgentScheduler | TaskType 较粗，很多任务落入 Custom | Typed Task intake + 权限预设 + 产物契约 |
| Orchestrator | 已做退行检测和 parallel tracks | 写权降级、subagent sandbox、track 专用执行分支 |
| Scientist | 编排 LeanShapedTranslator + HybridLeanKernel | 结构化 claim 提取、证据链、Bridge 可解释审查 |
| LeanKernel | 命题图 DAG/冲突/悬空引用检查 | 改名或明确为 PropositionKernel / LeanLite |

---

## Stage 1 - v3.0.0-alpha.1: 协议冻结

### 目标

先冻结跨模块协议，避免 Graph、Dream、MDT、Task、Sandbox 各做一套局部模型。

### 实现要点

- 定义 `MemoryRegion`: `working`, `long_term`, `semantic`, `dream_bridge`, `archive_hot`, `archive_warm`, `archive_cold`, `archive_frozen`。
- 定义 `MdtNodeMeta`: `mdt_version/id/title/slug/tree/tags/area/importance/summary/links/storage/content_hash`。
- 定义 `GraphEdgeType`: `parent`, `reference`, `related`, `source`, `sequence`, `semantic`, `temporal`, `bridge`。
- 定义 `TaskIntentType`: `research`, `summarize`, `verify`, `dream`, `mdt_index`, `mdt_read`, `mdt_pack`, `write_proposal`, `external_import`, `code_assist`。
- 定义 `SandboxPolicy`: `tool_allowlist`, `read_roots`, `write_roots`, `network_policy`, `max_runtime_secs`, `requires_bridge`.

expect:
  - symbol: "FeroHa v3 protocol model"
    file: "src-tauri/src/mdt/, src-tauri/src/graph/link_graph.rs, src-tauri/src/ai/agent_scheduler.rs"
    assert:
      - "Graph/Dream/MDT/Task/Sandbox 共享枚举和序列化契约"
      - "旧 wikilink 图在无 MDT 元数据时继续工作"
      - "所有新增协议类型有 Rust serde 测试和 TypeScript 类型镜像"
    source: "design:v3.0.0-alpha.1"
    confidence: 0.86

---

## Stage 2 - v3.0.0-alpha.2: MDT Reader / Indexer / Archive

### 目标

实现 MDT v0.1 的最小闭环：普通 Markdown 文件可以被识别、校验、索引、分层读取，并可打包为 `.mdtz`。

### 实现要点

- 扩展 frontmatter parser，使旧字段 `title/tags/aliases` 向后兼容。
- 新增 MDT indexer：扫描 vault，生成节点索引、边表、基础 metrics。
- 新增 Reader L0-L3：
  - L0: `id/title/tree/area/tags/importance/storage`
  - L1: L0 + `summary/links/headings`
  - L2: L1 + 相关段落
  - L3: 全文
- 新增 `.mdtz` pack/unpack，采用 ZIP + 固定根 `manifest.json`。
- 新增 FeroHa 指令卡 `feroha-mdt-reader.skill.md`，约束 agent 如何读取、写回、审计。

expect:
  - symbol: "MdtReader::load_context"
    file: "src-tauri/src/mdt/reader.rs"
    assert:
      - "按 query、tree、links、area、importance 排序候选节点"
      - "按 token budget 从 L0-L3 逐步展开"
      - "输出每个节点的读取层级和选择理由"
    source: "research:deep-research-report.md"
    confidence: 0.88

expect:
  - symbol: "MdtArchive"
    file: "src-tauri/src/mdt/archive.rs"
    assert:
      - "pack 输出 .mdtz"
      - "unpack 还原 manifest/nodes/assets/edges/indexes/skills/logs"
      - "不把 archive 当作唯一真源，源文件仍是普通 Markdown/MDT 节点"
    source: "research:deep-research-report.md"
    confidence: 0.82

---

## Stage 3 - v3.0.0-alpha.3: Dream-aware GraphView

### 目标

把 GraphView 从“笔记链接图”升级为“AI 面记忆结构图”。不同记忆分块和连接类型必须在前端有可识别的视觉表达。

### 视觉协议

| 关系类型 | 线型 | 语义 |
|---|---|---|
| `parent` | 实线 | 树状归属 |
| `reference` | 虚线 | 普通引用 |
| `related` | 点线 | 弱相关 |
| `source` | 虚线 + 箭头 | 证据来源 |
| `sequence` | 虚线 + 序号 | 流程/顺序 |
| `semantic` | 细线 + 低透明度 | 语义相似 |
| `temporal` | 细线 + 时间色阶 | 时间接近 |
| `bridge` | 高亮曲线 | Dream REM 桥接 |

### 实现要点

- `GraphEdge` 增加 `edge_type`, `origin`, `confidence`, `weight`, `memory_region`。
- `GraphNode` 增加 `area`, `storage_tier`, `memory_region`, `salience`, `community_id`。
- DreamEngine 的 `ConnectionType` 映射到 GraphEdgeType。
- GraphView 增加图例、类型过滤、Dream bridge 开关、storage tier 开关。
- Focus activation 保留，但叠加 Dream salience。

expect:
  - symbol: "GraphView Dream rendering"
    file: "src/components/GraphView.tsx"
    assert:
      - "不同 edge_type 使用不同线型"
      - "bridge/semantic/temporal 连接来自 Dream 或 MDT 索引"
      - "图例不是装饰文本，而是可操作过滤器"
    source: "design:v3.0.0-alpha.3"
    confidence: 0.84

---

## Stage 4 - v3.0.0-alpha.4: Typed Task Intake

### 目标

优化人类向 AI 面提 task 的入口：提交前先选择任务类型，任务类型决定工具权限、Bridge 风险等级、输出格式与默认 agent 路径。

### 任务类型

| 类型 | 默认工具 | 默认写权 | Bridge 风险 |
|---|---|---|---|
| `research` | vector, fulltext, web, papers, llm | proposal only | medium |
| `summarize` | read, vector, llm | proposal only | low |
| `verify` | read, vector, scientist, proposition_kernel | none | low |
| `dream` | dream_engine, graph_index | proposal only | medium |
| `mdt_index` | read vault, write indexes | generated files only | medium |
| `mdt_read` | read indexes, read notes | none | low |
| `mdt_pack` | read vault, write archive | archive only | medium |
| `write_proposal` | llm, ghost_store | ghost only | medium |
| `external_import` | network, parser, bridge | proposal only | high |
| `code_assist` | restricted file read, output_hook | proposal only | high |

expect:
  - symbol: "TaskIntentType"
    file: "src-tauri/src/ai/agent_scheduler.rs"
    assert:
      - "submit_task 不再把未知任务全部降级为 Custom"
      - "每个 task type 有默认 sandbox policy"
      - "BridgeProposal 展示 task type、scope、expected output 和 risk"
    source: "design:v3.0.0-alpha.4"
    confidence: 0.83

---

## Stage 5 - v3.0.0-beta.1: Sandboxed Orchestrator

### 目标

从代码层面落实 Orchestrator 原则：Orchestrator 是主体编排器，不是文件编辑器。它把材料、prompt、约束和验收标准交给 subagent，subagent 在受限 sandbox 中运行。

### 实现要点

- 新增 `SandboxPolicy` 与 `ToolCapability`。
- ToolRegistry 每次执行前检查 policy。
- Subagent job 必须携带 `read_roots/write_roots/network_policy/tool_allowlist`。
- Orchestrator 不持有 `write_note/save_note/apply_diff` 类能力。
- 所有源笔记写入必须经过 Bridge Proposal 或 Ghost/Diff。
- 参考 OpenAI Agents SDK sandbox 的模型：agent 通过受控工具执行 `execute/read_file/write_file/list_files`，而不是拿到底层宿主能力。

expect:
  - symbol: "SandboxPolicy::allows"
    file: "src-tauri/src/ai/sandbox.rs"
    assert:
      - "禁止未授权工具调用"
      - "禁止越界读取和越界写入"
      - "高风险任务必须生成 BridgeProposal"
    source: "design:v3.0.0-beta.1"
    confidence: 0.81

---

## Stage 6 - v3.0.0-beta.2: Regression Algorithm x Dream

### 目标

重写 agent 退行算法的核心判断：epoch 是否结束，不只看重复度和长度收敛，而要看记忆探索是否还产生有效信息增益。

### Epoch 结束条件

| 指标 | 结束信号 |
|---|---|
| novelty_delta | 连续 2 个 epoch 新 claim 比例低于阈值 |
| evidence_gain | 新证据来源不再增加或来源质量下降 |
| contradiction_risk | 新增矛盾密度超过阈值 |
| dream_coverage | 相关 Dream community 已覆盖到目标比例 |
| salience_shift | 低 salience 节点无法再提升问题解释力 |
| tool_loop | 同类工具重复调用且无新增 evidence |
| budget | token/time/iteration 达上限 |
| human_interrupt | 人类要求停止或转向 |

### 对“不断退行是否可以筛选信息”的判断

可以，但前提是退行被定义为受控筛选过程，而不是盲目重复生成。每次退行必须把信息放入 `candidate`, `kept`, `discarded`, `conflicted`, `needs_human` 五类之一，并记录原因与来源。

expect:
  - symbol: "Orchestrator::audit_epoch"
    file: "src-tauri/src/harness/orchestrator.rs"
    assert:
      - "退行判断读取 Dream community 和 salience"
      - "epoch end 输出明确 reason code"
      - "重复退行产生可审计的信息筛选记录"
    source: "design:v3.0.0-beta.2"
    confidence: 0.79

---

## Stage 7 - v3.0.0-beta.3: PropositionKernel / LeanLite

### 目标

保留当前 Lean 核心的工程价值，但重新命名和定位，避免把结构一致性误称为形式化真值证明。

### 决策

当前核心建议保留，并改名为 `PropositionKernel` 或在 UI/文档中标注 `LeanLite`。它负责：

- 命题图是否成环。
- 依赖是否悬空。
- 直接冲突是否存在。
- 证据链是否缺少 foundation。
- Scientist 输出是否足够进入 Bridge Proposal。

它不负责：

- 判断自然语言 claim 是否真实。
- 替代引用来源。
- 替代人工批准。
- 冒充真实 Lean theorem prover。

expect:
  - symbol: "PropositionKernel"
    file: "src-tauri/src/harness/lean_kernel.rs"
    assert:
      - "当前 HybridLeanKernel 行为保持兼容"
      - "新增 failure severity 和 repair hint"
      - "ScientistResult 暴露证据链和 kernel violations"
    source: "design:v3.0.0-beta.3"
    confidence: 0.84

---

## Stage 8 - v3.0.0-rc.1: Integration Hardening

### 目标

把 3.0.0 的新协议从“单点功能”打通成可发布工作流。

### 验收场景

1. 打开旧 vault，旧 wikilink 图正常显示。
2. 对旧 vault 执行 MDT index，生成 `edges.json` 与 `indexes/nodes.json`。
3. GraphView 可切换 wikilink、MDT、Dream 三类边。
4. 人类提交 `research` task，系统显示 task type、工具权限和 Bridge 风险。
5. DeepResearch 完成后，Scientist 提取 claim，PropositionKernel 验证，Bridge 生成审查提案。
6. Dream cycle 结束后，GraphView 出现 bridge/semantic/temporal 连接。
7. Orchestrator spawned tracks 使用受限 sandbox，不直接写源笔记。
8. `.mdtz` pack/unpack 后 manifest 与 content_hash 校验通过。

contract:
  - "v3.0.0 不破坏 v2.14.9 的核心使用路径"
  - "AI 面新增能力默认进入 Bridge，而不是静默写入"
  - "MDT v0.1 以文件系统为真源，索引是可重建缓存"
  - "Dream 连接可视化必须有真实数据来源"
  - "Orchestrator 无直接源笔记写权"
  - "Lean 核心以 PropositionKernel/LeanLite 身份保留"

