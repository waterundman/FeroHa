# v2.14.9 Bridge 闭环硬化与审查导航执行计划

## 目标

把 v2.14.8 已经建立的 Bridge Proposal 从“可看、可点”推进到“可追踪、可导航、可闭环”。本轮聚焦 C 方向：人类面与 AI 面之间的交互边界、审查入口、动作语义和失败隔离。

## 用户体验原则

- Bridge Inbox 不只是消息列表，而是人类审查 AI 意图的中控台。
- 所有“打开审查”的动作必须把用户带到真实工作面板，不能只返回一个后端成功值。
- 所有“拒绝/批准”的动作必须同时更新 Bridge 卡片和背后的 Task/Ghost 源对象。
- AI 自动生成 Proposal 失败时，不能阻断用户原本提交任务或运行命令的主流程。

## 执行步骤

1. 后端动作语义硬化
   - 收窄 `update_bridge_proposal_status`，只允许直接归档。
   - `reject` 对 Task 和 Ghost 都下沉到源对象。
   - `open_diff` / `open_trace` 返回 `target_panel` 与 `target_id` metadata。

2. 前端审查导航
   - `executeBridgeAction` 识别 `metadata.effect = navigate`。
   - 根据 `target_panel` 切换到 Diff 或 Tasks 面板。
   - Bridge 按钮增加进行中状态，避免重复点击触发多次后端动作。

3. Proposal 更新事件
   - 后端在创建或更新 Proposal 后发出 `bridge-proposal-updated`。
   - Bridge Inbox 监听事件并刷新列表。
   - Proposal 写入改为 best-effort，不影响原始 Task/CLI/Dream/Ghost 流程。

4. 来源补齐
   - 检查 Ghost、Dream、Scheduler、CLI 路径，补齐缺失的 Proposal 入口。
   - 保持每类来源的 summary、evidence、impact 与 action 命名一致。

5. 版本与验证
   - 版本推进到 `2.14.9`。
   - 更新项目仪表盘。
   - 跑通 Rust 单测、前端单测、前端构建和必要的编译检查。

## 验收标准

- Bridge 中点击打开 Diff/Trace 会跳转到对应工作面板。
- 重复点击同一动作不会产生重复执行。
- 拒绝 Ghost Proposal 会把 Ghost 标记为 rejected。
- 直接状态更新不能绕过动作语义写入 approved/rejected/applied。
- Proposal 写入失败不会阻断原任务提交。
- 所有新增行为都有对应测试或编译验证覆盖。
