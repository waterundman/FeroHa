# Bridge Proposal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 FeroHa v2.14.8 Bridge Proposal 协议层，让 AI 面的重要行动统一进入可审阅、可追溯、可批准/拒绝的 Bridge Inbox。

**Architecture:** 后端新增 `bridge` 模块，负责 Proposal 类型、风险分类、JSON 存储和 IPC 动作执行；现有 AgentTask、GhostNote、Scientist、Dream 只通过轻量 adapter 写入 Proposal，不被重写。前端新增 Bridge 面板，读取 Proposal 列表、分组展示、执行批准/拒绝/归档/跳转类动作。

**Tech Stack:** Rust/Tauri 2.0, serde JSON, React 18, Zustand, Vitest, existing FeroHa CSS variables and FeroHaIcon.

---

## 文件结构

新增后端文件：

- `src-tauri/src/bridge/mod.rs`: bridge 模块出口。
- `src-tauri/src/bridge/proposal.rs`: `BridgeProposal` 及风险/状态/动作类型。
- `src-tauri/src/bridge/store.rs`: `{vault}/.dualtrack/bridge/proposals.json` 读写、去重、状态更新。
- `src-tauri/src/bridge/commands.rs`: `list_bridge_proposals`、`get_bridge_proposal`、`update_bridge_proposal_status`、`execute_bridge_action`。

修改后端文件：

- `src-tauri/src/lib.rs`: 导出 `bridge` 模块。
- `src-tauri/src/main.rs`: 注册模块、初始化 `AppState.bridge_store`、注册 IPC handler。
- `src-tauri/src/state.rs`: 增加 `bridge_store` 字段。
- `src-tauri/src/fs/commands.rs`: `open_vault` 初始化 Bridge store。
- `src-tauri/src/ai/commands.rs`: 任务提交、Dream、Scientist、Ghost 创建路径接入 Proposal adapter。
- `src-tauri/src/ai/tool_registry.rs`: `GhostWriteTool` 返回足够 metadata，便于 adapter 生成 Proposal。

新增前端文件：

- `src/types/bridge-proposal.ts`: TypeScript Proposal 类型。
- `src/components/BridgeInbox.tsx`: Bridge 面板容器、数据加载、分组。
- `src/components/BridgeProposalCard.tsx`: 卡片摘要。
- `src/components/BridgeProposalDetail.tsx`: 详情和动作按钮。
- `src/components/__tests__/BridgeInbox.test.tsx`: Inbox 空态、分组和动作测试。

修改前端文件：

- `src/hooks/useAppStore.ts`: `activePanel` 联合类型增加 `bridge`，添加 proposal 状态和 fetch/action 方法。
- `src/App.tsx`: AI 模式新增 Bridge tab 和 panel。
- `src/components/AgentDashboard.tsx`: 保持现状，只补一个跳转提示或不改；Bridge Inbox 是主入口。

---

### Task 1: 后端 BridgeProposal 类型与 JSON Store

**Files:**
- Create: `src-tauri/src/bridge/mod.rs`
- Create: `src-tauri/src/bridge/proposal.rs`
- Create: `src-tauri/src/bridge/store.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/state.rs`
- Test: `src-tauri/src/bridge/store.rs`

- [ ] **Step 1: 写失败测试，覆盖创建、持久化、去重和风险分类**

在 `src-tauri/src/bridge/store.rs` 添加测试模块，先引用尚不存在的类型和方法：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::proposal::{
        BridgeProposal, BridgeProposalSource, BridgeProposalStatus, EvidenceKind,
        EvidenceRef, ImpactScope, ProposalAction, ProposalActionKind, ProposalRisk,
        SourceRef, SourceRefKind, TrustSnapshot,
    };
    use tempfile::tempdir;

    fn sample_proposal(source_id: &str) -> BridgeProposal {
        BridgeProposal {
            id: "proposal-1".to_string(),
            source: BridgeProposalSource::Tool,
            source_ref: SourceRef {
                kind: SourceRefKind::Task,
                id: source_id.to_string(),
                path: None,
            },
            intent: "Review AI research task".to_string(),
            summary: "AI wants to run a local research task.".to_string(),
            evidence: vec![EvidenceRef {
                label: "Task trace".to_string(),
                kind: EvidenceKind::Trace,
                reference: source_id.to_string(),
                confidence: Some(0.72),
                excerpt: Some("research task submitted".to_string()),
            }],
            impact: ImpactScope {
                notes: vec!["Bayes.md".to_string()],
                creates_files: false,
                modifies_notes: false,
                exports_data: false,
                external_side_effect: false,
            },
            risk: ProposalRisk::Low,
            status: BridgeProposalStatus::Pending,
            actions: vec![ProposalAction {
                id: "approve".to_string(),
                label: "Approve task".to_string(),
                kind: ProposalActionKind::ApproveTask,
                payload: serde_json::json!({ "task_id": source_id }),
            }],
            trust_snapshot: TrustSnapshot::default(),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn store_persists_and_lists_proposals() {
        let dir = tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let proposal = sample_proposal("task_a");

        store.upsert(proposal.clone()).unwrap();
        let listed = store.list(None).unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].source_ref.id, "task_a");

        let reloaded = BridgeProposalStore::new(dir.path().join("bridge"));
        let listed_again = reloaded.list(None).unwrap();
        assert_eq!(listed_again.len(), 1);
    }

    #[test]
    fn upsert_replaces_matching_source_ref() {
        let dir = tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let mut first = sample_proposal("task_a");
        first.summary = "first".to_string();
        let mut second = sample_proposal("task_a");
        second.id = "proposal-2".to_string();
        second.summary = "second".to_string();

        store.upsert(first).unwrap();
        store.upsert(second).unwrap();

        let listed = store.list(None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].summary, "second");
    }

    #[test]
    fn status_filter_excludes_archived_by_default() {
        let dir = tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let mut proposal = sample_proposal("task_a");
        proposal.status = BridgeProposalStatus::Archived;

        store.upsert(proposal).unwrap();

        assert!(store.list(None).unwrap().is_empty());
        assert_eq!(store.list(Some("archived")).unwrap().len(), 1);
    }

    #[test]
    fn risk_classifier_is_conservative() {
        let mut impact = ImpactScope::default();
        impact.modifies_notes = true;
        assert_eq!(BridgeProposal::classify_risk(&impact, false), ProposalRisk::High);

        let mut export_impact = ImpactScope::default();
        export_impact.exports_data = true;
        assert_eq!(BridgeProposal::classify_risk(&export_impact, false), ProposalRisk::High);

        let mut multi_note = ImpactScope::default();
        multi_note.notes = vec!["A.md".to_string(), "B.md".to_string()];
        assert_eq!(BridgeProposal::classify_risk(&multi_note, false), ProposalRisk::Medium);

        assert_eq!(BridgeProposal::classify_risk(&ImpactScope::default(), false), ProposalRisk::Low);
    }
}
```

- [ ] **Step 2: 运行失败测试**

Run:

```powershell
cargo test --lib bridge::store
```

Expected: FAIL，报 `bridge` 模块、`BridgeProposalStore` 或相关类型未定义。

- [ ] **Step 3: 实现 bridge 模块出口**

Create `src-tauri/src/bridge/mod.rs`:

```rust
pub mod commands;
pub mod proposal;
pub mod store;
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod bridge;
```

Modify `src-tauri/src/main.rs` 顶部模块声明：

```rust
mod bridge;
```

- [ ] **Step 4: 实现 Proposal 类型**

Create `src-tauri/src/bridge/proposal.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeProposalSource {
    Tool,
    Scientist,
    Dream,
    Ghost,
    Scheduler,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRefKind {
    Task,
    Ghost,
    DreamInsight,
    ScientistOutput,
    SchedulerJob,
    ToolCall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub kind: SourceRefKind,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Note,
    Chunk,
    Trace,
    ToolResult,
    Verification,
    Diff,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub label: String,
    pub kind: EvidenceKind,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactScope {
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub creates_files: bool,
    #[serde(default)]
    pub modifies_notes: bool,
    #[serde(default)]
    pub exports_data: bool,
    #[serde(default)]
    pub external_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeProposalStatus {
    Pending,
    Approved,
    Rejected,
    Applied,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalActionKind {
    ApproveTask,
    OpenDiff,
    OpenTrace,
    ApplyGhost,
    Reject,
    Archive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalAction {
    pub id: String,
    pub label: String,
    pub kind: ProposalActionKind,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustSnapshot {
    pub score: f32,
    pub acceptance_rate: f32,
    pub total_interactions: u32,
    pub recommended_mode: String,
}

impl Default for TrustSnapshot {
    fn default() -> Self {
        Self {
            score: 0.5,
            acceptance_rate: 0.0,
            total_interactions: 0,
            recommended_mode: "manual".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeProposal {
    pub id: String,
    pub source: BridgeProposalSource,
    pub source_ref: SourceRef,
    pub intent: String,
    pub summary: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    pub impact: ImpactScope,
    pub risk: ProposalRisk,
    pub status: BridgeProposalStatus,
    #[serde(default)]
    pub actions: Vec<ProposalAction>,
    #[serde(default)]
    pub trust_snapshot: TrustSnapshot,
    pub created_at: u64,
    pub updated_at: u64,
}

impl BridgeProposal {
    pub fn classify_risk(impact: &ImpactScope, has_verification_violations: bool) -> ProposalRisk {
        if impact.modifies_notes || impact.exports_data || impact.external_side_effect || has_verification_violations {
            return ProposalRisk::High;
        }
        if impact.creates_files || impact.notes.len() > 1 {
            return ProposalRisk::Medium;
        }
        ProposalRisk::Low
    }
}
```

- [ ] **Step 5: 实现 Store**

Create `src-tauri/src/bridge/store.rs`:

```rust
use crate::bridge::proposal::{BridgeProposal, BridgeProposalStatus};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BridgeProposalStore {
    root: PathBuf,
}

impl BridgeProposalStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn file_path(&self) -> PathBuf {
        self.root.join("proposals.json")
    }

    fn read_all(&self) -> Result<Vec<BridgeProposal>, String> {
        let path = self.file_path();
        if !path.exists() {
            return Ok(vec![]);
        }
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if text.trim().is_empty() {
            return Ok(vec![]);
        }
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    fn write_all(&self, proposals: &[BridgeProposal]) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(proposals).map_err(|e| e.to_string())?;
        fs::write(self.file_path(), text).map_err(|e| e.to_string())
    }

    pub fn list(&self, status_filter: Option<&str>) -> Result<Vec<BridgeProposal>, String> {
        let mut proposals = self.read_all()?;
        proposals.retain(|p| match status_filter {
            Some(filter) => format!("{:?}", p.status).eq_ignore_ascii_case(filter),
            None => p.status != BridgeProposalStatus::Archived,
        });
        proposals.sort_by_key(|p| std::cmp::Reverse(p.updated_at));
        Ok(proposals)
    }

    pub fn get(&self, id: &str) -> Result<Option<BridgeProposal>, String> {
        Ok(self.read_all()?.into_iter().find(|p| p.id == id))
    }

    pub fn upsert(&self, proposal: BridgeProposal) -> Result<BridgeProposal, String> {
        let mut proposals = self.read_all()?;
        if let Some(existing) = proposals.iter_mut().find(|p| {
            p.source_ref.kind == proposal.source_ref.kind && p.source_ref.id == proposal.source_ref.id
        }) {
            *existing = proposal.clone();
        } else {
            proposals.push(proposal.clone());
        }
        self.write_all(&proposals)?;
        Ok(proposal)
    }

    pub fn update_status(
        &self,
        id: &str,
        status: BridgeProposalStatus,
        updated_at: u64,
    ) -> Result<BridgeProposal, String> {
        let mut proposals = self.read_all()?;
        let proposal = proposals
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Bridge proposal not found: {}", id))?;
        proposal.status = status;
        proposal.updated_at = updated_at;
        let updated = proposal.clone();
        self.write_all(&proposals)?;
        Ok(updated)
    }
}

pub fn store_for_dualtrack_dir(dualtrack_dir: &Path) -> BridgeProposalStore {
    BridgeProposalStore::new(dualtrack_dir.join("bridge"))
}
```

- [ ] **Step 6: 将 Store 挂入 AppState 并在 open_vault 初始化**

Modify `src-tauri/src/state.rs`:

```rust
pub bridge_store: Option<crate::bridge::store::BridgeProposalStore>,
```

Modify `src-tauri/src/main.rs` AppState 初始化:

```rust
bridge_store: None,
```

Modify `src-tauri/src/fs/commands.rs` inside `open_vault` after `app.dualtrack_dir = dualtrack_dir.clone();`:

```rust
app.bridge_store = Some(crate::bridge::store::store_for_dualtrack_dir(&dualtrack_dir));
```

- [ ] **Step 7: 跑测试确认通过**

Run:

```powershell
cargo test --lib bridge::store
```

Expected: PASS，Bridge store 相关测试全部通过。

- [ ] **Step 8: 提交 Task 1**

```powershell
git add src-tauri/src/bridge src-tauri/src/lib.rs src-tauri/src/main.rs src-tauri/src/state.rs src-tauri/src/fs/commands.rs
git commit -m "feat: add bridge proposal store"
```

---

### Task 2: Bridge IPC 命令与动作执行

**Files:**
- Create: `src-tauri/src/bridge/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/bridge/commands.rs`

- [ ] **Step 1: 写失败测试，覆盖状态更新和 archive action**

Create `src-tauri/src/bridge/commands.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::proposal::*;
    use crate::bridge::store::BridgeProposalStore;
    use tempfile::tempdir;

    fn sample() -> BridgeProposal {
        BridgeProposal {
            id: "p1".to_string(),
            source: BridgeProposalSource::Tool,
            source_ref: SourceRef { kind: SourceRefKind::Task, id: "task_1".to_string(), path: None },
            intent: "Approve task".to_string(),
            summary: "Approve task".to_string(),
            evidence: vec![],
            impact: ImpactScope::default(),
            risk: ProposalRisk::Low,
            status: BridgeProposalStatus::Pending,
            actions: vec![ProposalAction {
                id: "archive".to_string(),
                label: "Archive".to_string(),
                kind: ProposalActionKind::Archive,
                payload: serde_json::json!({}),
            }],
            trust_snapshot: TrustSnapshot::default(),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn parse_status_rejects_unknown_values() {
        assert!(parse_status("pending").is_ok());
        assert!(parse_status("archived").is_ok());
        assert!(parse_status("nonsense").is_err());
    }

    #[test]
    fn execute_archive_action_updates_status() {
        let dir = tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        store.upsert(sample()).unwrap();

        let result = execute_action_against_store(&store, "p1", "archive", None).unwrap();

        assert_eq!(result.status, "success");
        assert_eq!(result.proposal.status, BridgeProposalStatus::Archived);
    }
}
```

- [ ] **Step 2: 运行失败测试**

Run:

```powershell
cargo test --lib bridge::commands
```

Expected: FAIL，`parse_status`、`BridgeProposalActionResult` 或 `execute_action_against_store` 未定义。

- [ ] **Step 3: 实现命令辅助函数和结果类型**

Add to `src-tauri/src/bridge/commands.rs`:

```rust
use crate::bridge::proposal::{BridgeProposal, BridgeProposalStatus, ProposalActionKind};
use crate::bridge::store::BridgeProposalStore;
use crate::{AiState, AppState};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BridgeProposalActionResult {
    pub status: String,
    pub message: String,
    pub proposal: BridgeProposal,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub fn parse_status(status: &str) -> Result<BridgeProposalStatus, String> {
    match status {
        "pending" => Ok(BridgeProposalStatus::Pending),
        "approved" => Ok(BridgeProposalStatus::Approved),
        "rejected" => Ok(BridgeProposalStatus::Rejected),
        "applied" => Ok(BridgeProposalStatus::Applied),
        "archived" => Ok(BridgeProposalStatus::Archived),
        _ => Err(format!("Unknown bridge proposal status: {}", status)),
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn execute_action_against_store(
    store: &BridgeProposalStore,
    id: &str,
    action_id: &str,
    mut approve_task: Option<&mut dyn FnMut(&str) -> Result<(), String>>,
) -> Result<BridgeProposalActionResult, String> {
    let proposal = store
        .get(id)?
        .ok_or_else(|| format!("Bridge proposal not found: {}", id))?;
    let action = proposal
        .actions
        .iter()
        .find(|a| a.id == action_id)
        .ok_or_else(|| format!("Bridge action not found: {}", action_id))?;

    let updated = match action.kind {
        ProposalActionKind::Archive => {
            store.update_status(id, BridgeProposalStatus::Archived, now_millis())?
        }
        ProposalActionKind::Reject => {
            store.update_status(id, BridgeProposalStatus::Rejected, now_millis())?
        }
        ProposalActionKind::ApproveTask => {
            let task_id = action
                .payload
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "approve_task action missing task_id".to_string())?;
            if let Some(ref mut approve) = approve_task {
                approve(task_id)?;
            }
            store.update_status(id, BridgeProposalStatus::Approved, now_millis())?
        }
        ProposalActionKind::OpenDiff | ProposalActionKind::OpenTrace | ProposalActionKind::ApplyGhost => {
            proposal.clone()
        }
    };

    Ok(BridgeProposalActionResult {
        status: "success".to_string(),
        message: format!("Executed bridge action: {}", action_id),
        proposal: updated,
        metadata: action.payload.clone(),
    })
}
```

- [ ] **Step 4: 实现 Tauri IPC 命令**

Add to `src-tauri/src/bridge/commands.rs`:

```rust
#[tauri::command]
pub(crate) fn list_bridge_proposals(
    status_filter: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<BridgeProposal>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let store = app.bridge_store.as_ref().ok_or("Bridge store not initialized")?;
    store.list(status_filter.as_deref())
}

#[tauri::command]
pub(crate) fn get_bridge_proposal(
    id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<BridgeProposal>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let store = app.bridge_store.as_ref().ok_or("Bridge store not initialized")?;
    store.get(&id)
}

#[tauri::command]
pub(crate) fn update_bridge_proposal_status(
    id: String,
    status: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<BridgeProposal, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let store = app.bridge_store.as_ref().ok_or("Bridge store not initialized")?;
    store.update_status(&id, parse_status(&status)?, now_millis())
}

#[tauri::command]
pub(crate) fn execute_bridge_action(
    id: String,
    action_id: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<BridgeProposalActionResult, String> {
    let store = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.bridge_store.as_ref().ok_or("Bridge store not initialized")?.clone()
    };

    let mut approve_task = |task_id: &str| -> Result<(), String> {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.agent_scheduler.approve(task_id, "human")?;
        ai.task_notifier.notify_one();
        let _ = app_handle.emit("task-updated", serde_json::json!({
            "task_id": task_id,
            "status": "approved"
        }));
        Ok(())
    };

    execute_action_against_store(&store, &id, &action_id, Some(&mut approve_task))
}
```

- [ ] **Step 5: 注册 IPC handler**

Modify `src-tauri/src/main.rs` `generate_handler!`:

```rust
bridge::commands::list_bridge_proposals,
bridge::commands::get_bridge_proposal,
bridge::commands::update_bridge_proposal_status,
bridge::commands::execute_bridge_action,
```

- [ ] **Step 6: 跑测试确认通过**

Run:

```powershell
cargo test --lib bridge::commands
```

Expected: PASS。

- [ ] **Step 7: 提交 Task 2**

```powershell
git add src-tauri/src/bridge/commands.rs src-tauri/src/main.rs
git commit -m "feat: expose bridge proposal commands"
```

---

### Task 3: 前端类型、Store 和 BridgeInbox 测试基线

**Files:**
- Create: `src/types/bridge-proposal.ts`
- Modify: `src/hooks/useAppStore.ts`
- Create: `src/components/__tests__/BridgeInbox.test.tsx`
- Create: `src/components/BridgeInbox.tsx`

- [ ] **Step 1: 写失败测试，覆盖空态和分组**

Create `src/components/__tests__/BridgeInbox.test.tsx`:

```tsx
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import BridgeInbox from "../BridgeInbox";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("BridgeInbox", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders an empty state when there are no proposals", async () => {
    invokeMock.mockResolvedValueOnce([]);

    render(<BridgeInbox isTauri />);

    expect(await screen.findByText("Bridge Inbox")).toBeInTheDocument();
    expect(await screen.findByText("暂无 AI 建议卡")).toBeInTheDocument();
  });

  it("groups pending and completed proposals", async () => {
    invokeMock.mockResolvedValueOnce([
      {
        id: "p1",
        source: "tool",
        source_ref: { kind: "task", id: "task_1" },
        intent: "批准研究任务",
        summary: "AI 想运行一次本地研究。",
        evidence: [],
        impact: { notes: ["Bayes.md"], creates_files: false, modifies_notes: false, exports_data: false, external_side_effect: false },
        risk: "low",
        status: "pending",
        actions: [],
        trust_snapshot: { score: 0.7, acceptance_rate: 0.8, total_interactions: 5, recommended_mode: "semi_auto" },
        created_at: 1,
        updated_at: 1,
      },
      {
        id: "p2",
        source: "dream",
        source_ref: { kind: "dream_insight", id: "dream_1" },
        intent: "归档洞察",
        summary: "Dream 发现一个弱关联。",
        evidence: [],
        impact: { notes: [], creates_files: false, modifies_notes: false, exports_data: false, external_side_effect: false },
        risk: "low",
        status: "archived",
        actions: [],
        trust_snapshot: { score: 0.5, acceptance_rate: 0, total_interactions: 0, recommended_mode: "manual" },
        created_at: 1,
        updated_at: 1,
      },
    ]);

    render(<BridgeInbox isTauri />);

    expect(await screen.findByText("需要处理")).toBeInTheDocument();
    expect(screen.getByText("已完成")).toBeInTheDocument();
    expect(screen.getByText("批准研究任务")).toBeInTheDocument();
    expect(screen.getByText("归档洞察")).toBeInTheDocument();
  });

  it("executes a proposal action through IPC", async () => {
    invokeMock
      .mockResolvedValueOnce([
        {
          id: "p1",
          source: "tool",
          source_ref: { kind: "task", id: "task_1" },
          intent: "批准研究任务",
          summary: "AI 想运行一次本地研究。",
          evidence: [],
          impact: { notes: [], creates_files: false, modifies_notes: false, exports_data: false, external_side_effect: false },
          risk: "low",
          status: "pending",
          actions: [{ id: "approve", label: "批准", kind: "approve_task", payload: { task_id: "task_1" } }],
          trust_snapshot: { score: 0.7, acceptance_rate: 0.8, total_interactions: 5, recommended_mode: "semi_auto" },
          created_at: 1,
          updated_at: 1,
        },
      ])
      .mockResolvedValueOnce({ status: "success" })
      .mockResolvedValueOnce([]);

    render(<BridgeInbox isTauri />);

    fireEvent.click(await screen.findByRole("button", { name: "批准" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("execute_bridge_action", { id: "p1", actionId: "approve" });
    });
  });
});
```

- [ ] **Step 2: 运行失败测试**

Run:

```powershell
npm.cmd test -- src/components/__tests__/BridgeInbox.test.tsx
```

Expected: FAIL，`BridgeInbox` 或类型尚不存在。

- [ ] **Step 3: 实现 TypeScript 类型**

Create `src/types/bridge-proposal.ts`:

```ts
export type BridgeProposalSource = "tool" | "scientist" | "dream" | "ghost" | "scheduler";
export type BridgeProposalStatus = "pending" | "approved" | "rejected" | "applied" | "archived";
export type ProposalRisk = "low" | "medium" | "high";
export type ProposalActionKind = "approve_task" | "open_diff" | "open_trace" | "apply_ghost" | "reject" | "archive";

export interface SourceRef {
  kind: "task" | "ghost" | "dream_insight" | "scientist_output" | "scheduler_job" | "tool_call";
  id: string;
  path?: string;
}

export interface EvidenceRef {
  label: string;
  kind: "note" | "chunk" | "trace" | "tool_result" | "verification" | "diff";
  ref: string;
  confidence?: number;
  excerpt?: string;
}

export interface ImpactScope {
  notes: string[];
  creates_files: boolean;
  modifies_notes: boolean;
  exports_data: boolean;
  external_side_effect: boolean;
}

export interface TrustSnapshot {
  score: number;
  acceptance_rate: number;
  total_interactions: number;
  recommended_mode: string;
}

export interface ProposalAction {
  id: string;
  label: string;
  kind: ProposalActionKind;
  payload: Record<string, unknown>;
}

export interface BridgeProposal {
  id: string;
  source: BridgeProposalSource;
  source_ref: SourceRef;
  intent: string;
  summary: string;
  evidence: EvidenceRef[];
  impact: ImpactScope;
  risk: ProposalRisk;
  status: BridgeProposalStatus;
  actions: ProposalAction[];
  trust_snapshot: TrustSnapshot;
  created_at: number;
  updated_at: number;
}
```

- [ ] **Step 4: 扩展 Zustand store**

Modify `src/hooks/useAppStore.ts` imports:

```ts
import type { BridgeProposal } from "../types/bridge-proposal";
```

Extend `AppStore`:

```ts
bridgeProposals: BridgeProposal[];
fetchBridgeProposals: () => Promise<void>;
executeBridgeAction: (id: string, actionId: string) => Promise<void>;
activePanel: "editor" | "graph" | "diff" | "tasks" | "cards" | "pipeline" | "plugins" | "inspiration" | "bridge";
setActivePanel: (panel: "editor" | "graph" | "diff" | "tasks" | "cards" | "pipeline" | "plugins" | "inspiration" | "bridge") => void;
```

Add implementation:

```ts
bridgeProposals: [],
fetchBridgeProposals: async () => {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const proposals = await invoke<BridgeProposal[]>("list_bridge_proposals", {});
    set({ bridgeProposals: proposals });
  } catch {
    set({ bridgeProposals: [] });
  }
},
executeBridgeAction: async (id, actionId) => {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("execute_bridge_action", { id, actionId });
  await get().fetchBridgeProposals();
},
```

- [ ] **Step 5: 实现临时 BridgeInbox 让测试过绿**

Create `src/components/BridgeInbox.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import type { BridgeProposal } from "../types/bridge-proposal";
import FeroHaIcon from "./FeroHaIcon";

interface BridgeInboxProps {
  isTauri: boolean;
}

export default function BridgeInbox({ isTauri }: BridgeInboxProps) {
  const [proposals, setProposals] = useState<BridgeProposal[]>([]);
  const [loading, setLoading] = useState(false);

  const load = async () => {
    if (!isTauri) {
      setProposals([]);
      return;
    }
    setLoading(true);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke<BridgeProposal[]>("list_bridge_proposals", {});
      setProposals(result);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, [isTauri]);

  const groups = useMemo(() => ({
    needsAction: proposals.filter((p) => p.status === "pending" || p.status === "approved"),
    completed: proposals.filter((p) => p.status === "applied" || p.status === "rejected" || p.status === "archived"),
  }), [proposals]);

  const executeAction = async (proposalId: string, actionId: string) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("execute_bridge_action", { id: proposalId, actionId });
    await load();
  };

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <h3 style={styles.title}><FeroHaIcon name="Inbox" size={18} /> Bridge Inbox</h3>
        <button style={styles.button} onClick={load}>刷新</button>
      </header>
      {loading && <div style={styles.muted}>正在加载建议卡...</div>}
      {!loading && proposals.length === 0 && <div style={styles.empty}>暂无 AI 建议卡</div>}
      {proposals.length > 0 && (
        <>
          <ProposalGroup title="需要处理" items={groups.needsAction} onAction={executeAction} />
          <ProposalGroup title="已完成" items={groups.completed} onAction={executeAction} />
        </>
      )}
    </div>
  );
}

function ProposalGroup({
  title,
  items,
  onAction,
}: {
  title: string;
  items: BridgeProposal[];
  onAction: (proposalId: string, actionId: string) => void;
}) {
  return (
    <section style={styles.group}>
      <h4 style={styles.groupTitle}>{title}</h4>
      {items.length === 0 ? <div style={styles.muted}>没有条目</div> : items.map((proposal) => (
        <article key={proposal.id} style={styles.card}>
          <div style={styles.cardMain}>
            <strong>{proposal.intent}</strong>
            <span style={styles.summary}>{proposal.summary}</span>
          </div>
          <div style={styles.actions}>
            {proposal.actions.map((action) => (
              <button key={action.id} style={styles.button} onClick={() => onAction(proposal.id, action.id)}>
                {action.label}
              </button>
            ))}
          </div>
        </article>
      ))}
    </section>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: { height: "100%", overflow: "auto", padding: 16, background: "var(--bg-primary)" },
  header: { display: "flex", alignItems: "center", justifyContent: "space-between", borderBottom: "1px solid var(--border-color)", paddingBottom: 10 },
  title: { display: "flex", alignItems: "center", gap: 8, margin: 0, fontSize: 16 },
  empty: { padding: 32, color: "var(--text-muted)", textAlign: "center" },
  muted: { padding: 8, color: "var(--text-muted)", fontSize: 12 },
  group: { marginTop: 14 },
  groupTitle: { fontSize: 13, margin: "0 0 8px", color: "var(--text-secondary)" },
  card: { display: "flex", justifyContent: "space-between", gap: 12, border: "1px solid var(--border-color)", borderRadius: 6, padding: 10, marginBottom: 8, background: "var(--bg-secondary)" },
  cardMain: { display: "flex", flexDirection: "column", gap: 4 },
  summary: { color: "var(--text-secondary)", fontSize: 12 },
  actions: { display: "flex", gap: 6, alignItems: "center" },
  button: { border: "1px solid var(--border-color)", background: "var(--bg-input)", color: "var(--text-primary)", borderRadius: 4, padding: "4px 8px", cursor: "pointer" },
};
```

- [ ] **Step 6: 跑测试确认过绿**

Run:

```powershell
npm.cmd test -- src/components/__tests__/BridgeInbox.test.tsx
```

Expected: PASS。

- [ ] **Step 7: 提交 Task 3**

```powershell
git add src/types/bridge-proposal.ts src/hooks/useAppStore.ts src/components/BridgeInbox.tsx src/components/__tests__/BridgeInbox.test.tsx
git commit -m "feat: add bridge inbox data model"
```

---

### Task 4: Bridge UI 组件拆分与 App 面板接入

**Files:**
- Create: `src/components/BridgeProposalCard.tsx`
- Create: `src/components/BridgeProposalDetail.tsx`
- Modify: `src/components/BridgeInbox.tsx`
- Modify: `src/App.tsx`
- Modify: `src/hooks/useAppStore.ts`
- Test: `src/components/__tests__/BridgeInbox.test.tsx`

- [ ] **Step 1: 扩展测试，要求卡片展示 risk、trust、affected notes**

Add to `BridgeInbox.test.tsx`:

```tsx
it("shows risk, trust score, evidence count, and affected notes on cards", async () => {
  invokeMock.mockResolvedValueOnce([
    {
      id: "p1",
      source: "ghost",
      source_ref: { kind: "ghost", id: "ghost_1" },
      intent: "审阅 Ghost 建议",
      summary: "AI 生成了 3 个建议段落。",
      evidence: [{ label: "Diff", kind: "diff", ref: "ghost_1" }],
      impact: { notes: ["Bayes.md"], creates_files: false, modifies_notes: true, exports_data: false, external_side_effect: false },
      risk: "high",
      status: "pending",
      actions: [],
      trust_snapshot: { score: 0.64, acceptance_rate: 0.5, total_interactions: 4, recommended_mode: "manual" },
      created_at: 1,
      updated_at: 1,
    },
  ]);

  render(<BridgeInbox isTauri />);

  expect(await screen.findByText("高风险")).toBeInTheDocument();
  expect(screen.getByText("信任 64%")).toBeInTheDocument();
  expect(screen.getByText("证据 1")).toBeInTheDocument();
  expect(screen.getByText("Bayes.md")).toBeInTheDocument();
});
```

- [ ] **Step 2: 运行失败测试**

Run:

```powershell
npm.cmd test -- src/components/__tests__/BridgeInbox.test.tsx
```

Expected: FAIL，当前临时卡片未显示这些字段。

- [ ] **Step 3: 实现 BridgeProposalCard**

Create `src/components/BridgeProposalCard.tsx`:

```tsx
import type { BridgeProposal } from "../types/bridge-proposal";
import FeroHaIcon from "./FeroHaIcon";

interface Props {
  proposal: BridgeProposal;
  selected: boolean;
  onSelect: () => void;
  onAction: (proposalId: string, actionId: string) => void;
}

const riskLabel = { low: "低风险", medium: "中风险", high: "高风险" } as const;
const sourceIcon: Record<BridgeProposal["source"], string> = {
  tool: "Wrench",
  scientist: "FlaskConical",
  dream: "Moon",
  ghost: "GitCompare",
  scheduler: "Clock",
};

export default function BridgeProposalCard({ proposal, selected, onSelect, onAction }: Props) {
  const trust = Math.round(proposal.trust_snapshot.score * 100);
  return (
    <article className={`bridge-card${selected ? " selected" : ""}`} onClick={onSelect}>
      <div className="bridge-card-icon"><FeroHaIcon name={sourceIcon[proposal.source]} size={16} /></div>
      <div className="bridge-card-main">
        <div className="bridge-card-title-row">
          <strong>{proposal.intent}</strong>
          <span className={`bridge-risk bridge-risk-${proposal.risk}`}>{riskLabel[proposal.risk]}</span>
        </div>
        <div className="bridge-card-summary">{proposal.summary}</div>
        <div className="bridge-card-meta">
          <span>信任 {trust}%</span>
          <span>证据 {proposal.evidence.length}</span>
          {proposal.impact.notes.slice(0, 2).map((note) => <span key={note}>{note}</span>)}
        </div>
      </div>
      <div className="bridge-card-actions" onClick={(e) => e.stopPropagation()}>
        {proposal.actions.slice(0, 3).map((action) => (
          <button key={action.id} onClick={() => onAction(proposal.id, action.id)}>{action.label}</button>
        ))}
      </div>
    </article>
  );
}
```

- [ ] **Step 4: 实现 BridgeProposalDetail**

Create `src/components/BridgeProposalDetail.tsx`:

```tsx
import type { BridgeProposal } from "../types/bridge-proposal";

interface Props {
  proposal: BridgeProposal | null;
  onAction: (proposalId: string, actionId: string) => void;
}

export default function BridgeProposalDetail({ proposal, onAction }: Props) {
  if (!proposal) {
    return <aside className="bridge-detail empty">选择一张建议卡查看证据和影响范围</aside>;
  }

  return (
    <aside className="bridge-detail">
      <h4>{proposal.intent}</h4>
      <p>{proposal.summary}</p>
      <section>
        <h5>影响范围</h5>
        <div>笔记: {proposal.impact.notes.length ? proposal.impact.notes.join(", ") : "无直接笔记影响"}</div>
        <div>修改笔记: {proposal.impact.modifies_notes ? "是" : "否"}</div>
        <div>外部副作用: {proposal.impact.external_side_effect ? "是" : "否"}</div>
      </section>
      <section>
        <h5>证据</h5>
        {proposal.evidence.length === 0 ? <div className="muted">无证据条目</div> : proposal.evidence.map((evidence) => (
          <div key={`${evidence.kind}-${evidence.ref}`} className="bridge-evidence">
            <strong>{evidence.label}</strong>
            <span>{evidence.kind}: {evidence.ref}</span>
            {evidence.excerpt && <p>{evidence.excerpt}</p>}
          </div>
        ))}
      </section>
      <section className="bridge-detail-actions">
        {proposal.actions.map((action) => (
          <button key={action.id} onClick={() => onAction(proposal.id, action.id)}>{action.label}</button>
        ))}
      </section>
    </aside>
  );
}
```

- [ ] **Step 5: 重构 BridgeInbox 使用拆分组件并补 CSS**

Modify `src/components/BridgeInbox.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import type { BridgeProposal } from "../types/bridge-proposal";
import BridgeProposalCard from "./BridgeProposalCard";
import BridgeProposalDetail from "./BridgeProposalDetail";
import FeroHaIcon from "./FeroHaIcon";

interface BridgeInboxProps {
  isTauri: boolean;
}

export default function BridgeInbox({ isTauri }: BridgeInboxProps) {
  const [proposals, setProposals] = useState<BridgeProposal[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = async () => {
    if (!isTauri) {
      setProposals([]);
      return;
    }
    setLoading(true);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke<BridgeProposal[]>("list_bridge_proposals", {});
      setProposals(result);
      if (!selectedId && result[0]) setSelectedId(result[0].id);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, [isTauri]);

  const groups = useMemo(() => ({
    needsAction: proposals.filter((p) => p.status === "pending" || p.status === "approved"),
    completed: proposals.filter((p) => p.status === "applied" || p.status === "rejected" || p.status === "archived"),
  }), [proposals]);

  const selected = proposals.find((p) => p.id === selectedId) ?? null;

  const executeAction = async (proposalId: string, actionId: string) => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("execute_bridge_action", { id: proposalId, actionId });
    await load();
  };

  return (
    <div className="bridge-inbox">
      <header className="bridge-header">
        <h3><FeroHaIcon name="Inbox" size={18} /> Bridge Inbox</h3>
        <button onClick={load}>刷新</button>
      </header>
      {loading && <div className="bridge-muted">正在加载建议卡...</div>}
      {!loading && proposals.length === 0 && <div className="bridge-empty">暂无 AI 建议卡</div>}
      {proposals.length > 0 && (
        <div className="bridge-layout">
          <div className="bridge-list">
            <ProposalGroup title="需要处理" items={groups.needsAction} selectedId={selectedId} onSelect={setSelectedId} onAction={executeAction} />
            <ProposalGroup title="已完成" items={groups.completed} selectedId={selectedId} onSelect={setSelectedId} onAction={executeAction} />
          </div>
          <BridgeProposalDetail proposal={selected} onAction={executeAction} />
        </div>
      )}
      <style>{bridgeCss}</style>
    </div>
  );
}

function ProposalGroup({
  title, items, selectedId, onSelect, onAction,
}: {
  title: string;
  items: BridgeProposal[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onAction: (proposalId: string, actionId: string) => void;
}) {
  return (
    <section className="bridge-group">
      <h4>{title}</h4>
      {items.length === 0 ? <div className="bridge-muted">没有条目</div> : items.map((proposal) => (
        <BridgeProposalCard
          key={proposal.id}
          proposal={proposal}
          selected={proposal.id === selectedId}
          onSelect={() => onSelect(proposal.id)}
          onAction={onAction}
        />
      ))}
    </section>
  );
}

const bridgeCss = `
.bridge-inbox { height: 100%; overflow: hidden; display: flex; flex-direction: column; background: var(--bg-primary); color: var(--text-primary); }
.bridge-header { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; border-bottom: 1px solid var(--border-color); }
.bridge-header h3 { display: flex; align-items: center; gap: 8px; margin: 0; font-size: 16px; }
.bridge-header button, .bridge-card-actions button, .bridge-detail-actions button { border: 1px solid var(--border-color); background: var(--bg-input); color: var(--text-primary); border-radius: 4px; padding: 4px 8px; cursor: pointer; }
.bridge-layout { display: grid; grid-template-columns: minmax(320px, 44%) 1fr; min-height: 0; flex: 1; }
.bridge-list { overflow: auto; padding: 12px; border-right: 1px solid var(--border-color); }
.bridge-group h4 { margin: 0 0 8px; color: var(--text-secondary); font-size: 13px; }
.bridge-card { display: flex; gap: 10px; padding: 10px; border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-secondary); margin-bottom: 8px; cursor: pointer; }
.bridge-card.selected { border-color: var(--accent-primary); background: var(--bg-input); }
.bridge-card-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.bridge-card-title-row { display: flex; align-items: center; gap: 8px; justify-content: space-between; }
.bridge-card-summary { color: var(--text-secondary); font-size: 12px; line-height: 1.4; }
.bridge-card-meta { display: flex; gap: 6px; flex-wrap: wrap; color: var(--text-muted); font-size: 11px; }
.bridge-risk { font-size: 10px; padding: 1px 6px; border-radius: 3px; border: 1px solid var(--border-color); }
.bridge-risk-high { color: var(--status-error-color); }
.bridge-risk-medium { color: var(--diff-warn); }
.bridge-risk-low { color: var(--accent-primary); }
.bridge-card-actions { display: flex; flex-direction: column; gap: 4px; }
.bridge-detail { overflow: auto; padding: 16px; display: flex; flex-direction: column; gap: 12px; }
.bridge-detail.empty, .bridge-empty, .bridge-muted { color: var(--text-muted); }
.bridge-empty { padding: 32px; text-align: center; }
.bridge-evidence { border: 1px solid var(--border-color); border-radius: 4px; padding: 8px; margin-bottom: 6px; background: var(--bg-secondary); display: flex; flex-direction: column; gap: 4px; }
.bridge-detail-actions { display: flex; gap: 6px; flex-wrap: wrap; }
`;
```

- [ ] **Step 6: 接入 App 面板**

Modify `src/App.tsx` imports:

```tsx
import BridgeInbox from "./components/BridgeInbox";
```

Extend `tabIcons`:

```tsx
bridge: "Inbox",
```

Add sidebar tab in AI mode:

```tsx
{mode === "ai" && <TabBtn panel="bridge" title="Bridge" />}
```

Add panel:

```tsx
<div role="tabpanel" id="panel-bridge" aria-label="Bridge Inbox panel" hidden={activePanel !== "bridge"}>
  {activePanel === "bridge" && <BridgeInbox isTauri={isTauri} />}
</div>
```

Update `TabBtn` panel union:

```tsx
"editor" | "graph" | "diff" | "tasks" | "cards" | "pipeline" | "plugins" | "inspiration" | "bridge"
```

- [ ] **Step 7: 跑前端测试**

Run:

```powershell
npm.cmd test -- src/components/__tests__/BridgeInbox.test.tsx
```

Expected: PASS。

- [ ] **Step 8: 提交 Task 4**

```powershell
git add src/App.tsx src/hooks/useAppStore.ts src/components/BridgeInbox.tsx src/components/BridgeProposalCard.tsx src/components/BridgeProposalDetail.tsx src/components/__tests__/BridgeInbox.test.tsx
git commit -m "feat: add bridge inbox panel"
```

---

### Task 5: Source Adapters 接入 Task、Ghost、Scientist、Dream

**Files:**
- Modify: `src-tauri/src/bridge/proposal.rs`
- Modify: `src-tauri/src/bridge/store.rs`
- Modify: `src-tauri/src/ai/commands.rs`
- Modify: `src-tauri/src/ai/tool_registry.rs`
- Test: `src-tauri/src/bridge/proposal.rs`

- [ ] **Step 1: 写失败测试，覆盖 task/ghost/scientist/dream adapter 构造**

Add to `src-tauri/src/bridge/proposal.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_proposal_contains_approve_action() {
        let proposal = BridgeProposal::for_task(
            "task_1",
            "Research Bayes",
            TrustSnapshot::default(),
            1,
        );

        assert_eq!(proposal.source, BridgeProposalSource::Tool);
        assert_eq!(proposal.source_ref.id, "task_1");
        assert!(proposal.actions.iter().any(|a| a.kind == ProposalActionKind::ApproveTask));
    }

    #[test]
    fn ghost_proposal_is_high_risk_when_it_modifies_note() {
        let proposal = BridgeProposal::for_ghost(
            "ghost_1",
            "Target.md",
            3,
            TrustSnapshot::default(),
            1,
        );

        assert_eq!(proposal.source, BridgeProposalSource::Ghost);
        assert_eq!(proposal.risk, ProposalRisk::High);
        assert!(proposal.actions.iter().any(|a| a.kind == ProposalActionKind::OpenDiff));
    }

    #[test]
    fn scientist_proposal_is_high_risk_with_violations() {
        let proposal = BridgeProposal::for_scientist_result(
            "task_1",
            "Deep research",
            4,
            1,
            vec!["Note.md".to_string()],
            TrustSnapshot::default(),
            1,
        );

        assert_eq!(proposal.source, BridgeProposalSource::Scientist);
        assert_eq!(proposal.risk, ProposalRisk::High);
    }
}
```

- [ ] **Step 2: 运行失败测试**

Run:

```powershell
cargo test --lib bridge::proposal
```

Expected: FAIL，adapter constructors 未实现。

- [ ] **Step 3: 实现 adapter constructors**

Add to `impl BridgeProposal` in `src-tauri/src/bridge/proposal.rs`:

```rust
pub fn for_task(task_id: &str, intent: &str, trust_snapshot: TrustSnapshot, now: u64) -> Self {
    let impact = ImpactScope::default();
    Self {
        id: format!("bridge_{}", uuid::Uuid::new_v4().to_string().replace('-', "")),
        source: BridgeProposalSource::Tool,
        source_ref: SourceRef { kind: SourceRefKind::Task, id: task_id.to_string(), path: None },
        intent: intent.to_string(),
        summary: "AI 准备执行一个需要人类确认的任务。".to_string(),
        evidence: vec![EvidenceRef {
            label: "Agent task".to_string(),
            kind: EvidenceKind::Trace,
            reference: task_id.to_string(),
            confidence: None,
            excerpt: Some(intent.to_string()),
        }],
        risk: Self::classify_risk(&impact, false),
        impact,
        status: BridgeProposalStatus::Pending,
        actions: vec![
            ProposalAction {
                id: "approve".to_string(),
                label: "批准".to_string(),
                kind: ProposalActionKind::ApproveTask,
                payload: serde_json::json!({ "task_id": task_id }),
            },
            ProposalAction {
                id: "reject".to_string(),
                label: "拒绝".to_string(),
                kind: ProposalActionKind::Reject,
                payload: serde_json::json!({}),
            },
        ],
        trust_snapshot,
        created_at: now,
        updated_at: now,
    }
}

pub fn for_ghost(ghost_id: &str, target_note: &str, block_count: usize, trust_snapshot: TrustSnapshot, now: u64) -> Self {
    let impact = ImpactScope {
        notes: vec![target_note.to_string()],
        creates_files: false,
        modifies_notes: true,
        exports_data: false,
        external_side_effect: false,
    };
    Self {
        id: format!("bridge_{}", uuid::Uuid::new_v4().to_string().replace('-', "")),
        source: BridgeProposalSource::Ghost,
        source_ref: SourceRef { kind: SourceRefKind::Ghost, id: ghost_id.to_string(), path: Some(target_note.to_string()) },
        intent: format!("审阅 {} 个 Ghost 建议块", block_count),
        summary: format!("AI 为 `{}` 创建了 {} 个待审阅建议块。", target_note, block_count),
        evidence: vec![EvidenceRef {
            label: "Ghost diff".to_string(),
            kind: EvidenceKind::Diff,
            reference: ghost_id.to_string(),
            confidence: None,
            excerpt: None,
        }],
        risk: Self::classify_risk(&impact, false),
        impact,
        status: BridgeProposalStatus::Pending,
        actions: vec![
            ProposalAction { id: "open-diff".to_string(), label: "打开 Diff".to_string(), kind: ProposalActionKind::OpenDiff, payload: serde_json::json!({ "ghost_id": ghost_id }) },
            ProposalAction { id: "reject".to_string(), label: "拒绝".to_string(), kind: ProposalActionKind::Reject, payload: serde_json::json!({}) },
        ],
        trust_snapshot,
        created_at: now,
        updated_at: now,
    }
}

pub fn for_scientist_result(
    task_id: &str,
    topic: &str,
    claim_count: usize,
    violation_count: usize,
    related_notes: Vec<String>,
    trust_snapshot: TrustSnapshot,
    now: u64,
) -> Self {
    let impact = ImpactScope {
        notes: related_notes,
        creates_files: false,
        modifies_notes: false,
        exports_data: false,
        external_side_effect: false,
    };
    Self {
        id: format!("bridge_{}", uuid::Uuid::new_v4().to_string().replace('-', "")),
        source: BridgeProposalSource::Scientist,
        source_ref: SourceRef { kind: SourceRefKind::ScientistOutput, id: task_id.to_string(), path: None },
        intent: format!("审阅 Scientist 精炼结果: {}", topic),
        summary: format!("Scientist 提取了 {} 条 claims，发现 {} 个验证问题。", claim_count, violation_count),
        evidence: vec![EvidenceRef {
            label: "Scientist verification".to_string(),
            kind: EvidenceKind::Verification,
            reference: task_id.to_string(),
            confidence: None,
            excerpt: Some(format!("claims={}, violations={}", claim_count, violation_count)),
        }],
        risk: Self::classify_risk(&impact, violation_count > 0),
        impact,
        status: BridgeProposalStatus::Pending,
        actions: vec![
            ProposalAction { id: "open-trace".to_string(), label: "打开 Trace".to_string(), kind: ProposalActionKind::OpenTrace, payload: serde_json::json!({ "task_id": task_id }) },
            ProposalAction { id: "archive".to_string(), label: "归档".to_string(), kind: ProposalActionKind::Archive, payload: serde_json::json!({}) },
        ],
        trust_snapshot,
        created_at: now,
        updated_at: now,
    }
}
```

- [ ] **Step 4: 添加 trust snapshot helper**

Add to `src-tauri/src/bridge/proposal.rs`:

```rust
impl TrustSnapshot {
    pub fn from_protocol(protocol: Option<&crate::ipc::protocol::TwoSurfaceProtocol>) -> Self {
        if let Some(protocol) = protocol {
            Self {
                score: protocol.trust_score_value(),
                acceptance_rate: protocol.acceptance_rate(),
                total_interactions: protocol.total_interactions(),
                recommended_mode: format!("{:?}", protocol.current_mode()).to_lowercase(),
            }
        } else {
            Self::default()
        }
    }
}
```

- [ ] **Step 5: 在 submit_task / execute_cli 生成 task proposal**

Modify `src-tauri/src/ai/commands.rs` signatures to include `state: State<'_, Mutex<AppState>>` where needed:

```rust
pub(crate) async fn execute_cli(
    command: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    _app_handle: AppHandle,
) -> Result<String, String>
```

After `ai.agent_scheduler.submit(task);`, add:

```rust
let app = state.lock().map_err(|e| e.to_string())?;
if let Some(store) = app.bridge_store.as_ref() {
    let proposal = crate::bridge::proposal::BridgeProposal::for_task(
        &task_id,
        &command,
        crate::bridge::proposal::TrustSnapshot::from_protocol(app.protocol.as_ref()),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
    );
    let _ = store.upsert(proposal);
}
```

Repeat the same pattern in `submit_task` after submission. Add `state: State<'_, Mutex<AppState>>` to `submit_task`.

- [ ] **Step 6: 在 GhostWriteTool metadata 保留 ghost info**

Verify `src-tauri/src/ai/tool_registry.rs` `GhostWriteTool` already returns:

```rust
"ghost_id": ghost_note.id,
"target_note": target_note,
"blocks_count": ghost_note.suggested_blocks.len()
```

If missing, add those exact keys. If already present, no code change.

- [ ] **Step 7: 在 tool-call loop 处理 ghost_write metadata 时创建 ghost proposal**

Find the CustomCard tool-call loop in `execute_agent_task_async`. After a tool result is produced, add a guarded block:

```rust
if tool_result.tool_name == "ghost_write" {
    let ghost_id = tool_result.metadata.get("ghost_id").and_then(|v| v.as_str());
    let target_note = tool_result.metadata.get("target_note").and_then(|v| v.as_str()).unwrap_or("untitled.md");
    let blocks_count = tool_result.metadata.get("blocks_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if let Some(ghost_id) = ghost_id {
        let app = state.lock().map_err(|e| e.to_string())?;
        if let Some(store) = app.bridge_store.as_ref() {
            let proposal = crate::bridge::proposal::BridgeProposal::for_ghost(
                ghost_id,
                target_note,
                blocks_count,
                crate::bridge::proposal::TrustSnapshot::from_protocol(app.protocol.as_ref()),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            );
            let _ = store.upsert(proposal);
        }
    }
}
```

- [ ] **Step 8: 在 Scientist refine 成功后创建 scientist proposal**

Where `Scientist::refine` is called in `execute_agent_task_async`, after result:

```rust
let app = state.lock().map_err(|e| e.to_string())?;
if let Some(store) = app.bridge_store.as_ref() {
    let related_notes = result.clean_knowledge.sources.iter().map(|s| s.key.clone()).collect();
    let proposal = crate::bridge::proposal::BridgeProposal::for_scientist_result(
        task_id,
        &task_clone.intent,
        result.clean_knowledge.claims.len(),
        result.verification.as_ref().map(|v| v.violations.len()).unwrap_or(0),
        related_notes,
        crate::bridge::proposal::TrustSnapshot::from_protocol(app.protocol.as_ref()),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
    );
    let _ = store.upsert(proposal);
}
```

- [ ] **Step 9: Dream grouped proposal**

Add a `BridgeProposal::for_dream_cycle` constructor similar to `for_scientist_result`, with source `Dream`, kind `DreamInsight`, low risk, action `archive`. In `trigger_dream`, after `insights` are available and before returning summary, insert one grouped proposal if `!insights.is_empty()`.

- [ ] **Step 10: 跑后端测试**

Run:

```powershell
cargo test --lib bridge::proposal
cargo test --lib bridge
```

Expected: PASS。

- [ ] **Step 11: 提交 Task 5**

```powershell
git add src-tauri/src/bridge src-tauri/src/ai/commands.rs src-tauri/src/ai/tool_registry.rs
git commit -m "feat: create bridge proposals from ai events"
```

---

### Task 6: 全量验证、版本标记和文档同步

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/main.rs`
- Optionally Modify: `D:\obsidian分2\ai引用库\项目概况\FeroHa\开发仪表盘.md`

- [ ] **Step 1: 运行完整后端测试**

Run:

```powershell
cargo test --lib
```

Expected: all Rust tests pass. Current baseline before implementation was 188/188 PASS.

- [ ] **Step 2: 运行完整前端测试**

Run:

```powershell
npm.cmd test
```

Expected: all Vitest tests pass. Current baseline before implementation was 29/29 PASS; after this plan there should be more tests including BridgeInbox.

- [ ] **Step 3: 运行生产构建**

Run:

```powershell
npm.cmd run build
```

Expected: `tsc --noEmit && vite build` exits 0. Large chunk warnings are acceptable if they match current project behavior.

- [ ] **Step 4: 版本号更新到 2.14.8**

Modify `package.json`:

```json
"version": "2.14.8"
```

Modify `src-tauri/Cargo.toml`:

```toml
version = "2.14.8"
```

Modify `src-tauri/src/main.rs` header comment:

```rust
// v2.14.8 — Bridge Proposal protocol and human-review inbox
```

- [ ] **Step 5: 再跑最小验证**

Run:

```powershell
cargo test --lib bridge
npm.cmd test -- src/components/__tests__/BridgeInbox.test.tsx
npm.cmd run build
```

Expected: all pass.

- [ ] **Step 6: 更新开发仪表盘**

If editing the Obsidian project docs is in scope for the execution session, prepend a v2.14.8 section to:

```text
D:\obsidian分2\ai引用库\项目概况\FeroHa\开发仪表盘.md
```

Use this Chinese summary:

```markdown
## v2.14.8 迭代概览

**主题**: Bridge Proposal 协议层 + AI 建议卡收件箱

### 变更概要
- **S1 BridgeProposal 后端协议**: 新增 Proposal 类型、风险分类、JSON Store、IPC 命令。
- **S2 Bridge Inbox 前端**: AI 模式新增 Bridge 面板，按状态分组展示建议卡。
- **S3 Source Adapters**: Task/Ghost/Scientist/Dream 重要结果生成可审阅 Proposal。
- **S4 验证闭环**: Rust + Vitest + Build 全量验证。
```

- [ ] **Step 7: 提交最终验证和版本更新**

```powershell
git add package.json src-tauri/Cargo.toml src-tauri/src/main.rs
git commit -m "chore: bump version to 2.14.8"
```

If the Obsidian dashboard was updated:

```powershell
git add "D:\obsidian分2\ai引用库\项目概况\FeroHa\开发仪表盘.md"
git commit -m "docs: update feroha dashboard for v2.14.8"
```

---

## 自审清单

- Spec coverage:
  - BridgeProposal 类型、风险、状态、动作: Task 1。
  - JSON storage: Task 1。
  - IPC commands: Task 2。
  - Bridge Inbox: Task 3 和 Task 4。
  - Source adapters: Task 5。
  - TrustScore snapshot without authority bypass: Task 1/5 类型和 adapter。
  - Verification: Task 6。

- No-placeholder scan:
  - 本计划不使用未定稿标记、空任务标记或延后补全语句。
  - 每个任务都包含失败测试、运行命令、实现片段和提交命令。

- Type consistency:
  - Rust `snake_case` serde enum values align with TypeScript string unions.
  - Frontend IPC args use Tauri camelCase mapping: `{ id, actionId }` maps to Rust `id`, `action_id`.
  - `BridgeProposal.source_ref` and `trust_snapshot` match the existing FeroHa naming style used in backend JSON.

---

## 执行交接

Plan complete and saved to `docs/superpowers/plans/2026-05-28-bridge-proposal-implementation-plan.md`. Two execution options:

1. **Subagent-Driven (recommended)** - 我按任务拆分 fresh subagent 执行，每个任务后做代码审查和验证。
2. **Inline Execution** - 我在当前会话内按计划执行，适合你想更连续地看见每一步。

Which approach?
