# FeroHa v3 Memory Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build FeroHa v3.0.0 as a memory-architecture release: MDT reader/indexer, Dream-aware graph rendering, typed task intake, sandboxed subagents, Dream-linked regression audits, and PropositionKernel verification.

**Architecture:** Add MDT as a focused Rust module that reads ordinary Markdown/frontmatter and emits generated indexes. Extend the existing graph, Dream, task, Bridge, and harness modules through shared serde types instead of parallel ad hoc models. Keep human write authority intact by routing AI writes through Ghost/Diff or Bridge Proposal.

**Tech Stack:** Tauri 2, Rust, serde/serde_yaml/serde_json, rusqlite, zip, React 18, TypeScript, canvas GraphView, existing Vitest and Rust test suites.

---

## File Structure

- Create: `src-tauri/src/mdt/mod.rs` - MDT module exports.
- Create: `src-tauri/src/mdt/types.rs` - shared MDT node, edge, reader, archive, and storage types.
- Create: `src-tauri/src/mdt/reader.rs` - L0-L3 context loading.
- Create: `src-tauri/src/mdt/indexer.rs` - vault scan, edges, node index, metrics index.
- Create: `src-tauri/src/mdt/archive.rs` - `.mdtz` pack/unpack.
- Create: `src-tauri/src/ai/sandbox.rs` - task sandbox policy and capability checks.
- Create: `src-tauri/src/ai/task_intent.rs` - typed task intake contract.
- Create: `src-tauri/src/harness/regression.rs` - Dream-aware epoch metrics.
- Create: `src-tauri/src/harness/proposition_kernel.rs` - compatibility wrapper around current `lean_kernel`.
- Create: `src-tauri/src/mdt/feroha-mdt-reader.skill.md` - agent command-card usage guide.
- Modify: `src-tauri/src/parser/frontmatter.rs` - parse legacy frontmatter plus MDT fields.
- Modify: `src-tauri/src/graph/link_graph.rs` - typed edges and memory metadata.
- Modify: `src-tauri/src/graph/commands.rs` - graph IPC returns typed graph data.
- Modify: `src-tauri/src/ai/dream_engine.rs` - expose Dream graph projection.
- Modify: `src-tauri/src/ai/agent_scheduler.rs` - typed task and sandbox defaults.
- Modify: `src-tauri/src/ai/tool_registry.rs` - enforce sandbox capability checks.
- Modify: `src-tauri/src/ai/commands.rs` - task submission, MDT commands, Bridge metadata.
- Modify: `src-tauri/src/harness/orchestrator.rs` - use Dream-aware regression result codes.
- Modify: `src-tauri/src/harness/scientist.rs` - emit structured claims and evidence chains.
- Modify: `src-tauri/src/harness/lean_kernel.rs` - keep compatibility re-export or internal wrapper.
- Modify: `src/components/GraphView.tsx` - render typed edges and Dream memory regions.
- Modify: `src/components/CliBar.tsx` and `src/components/CommandCardPanel.tsx` - typed task selection.
- Modify: `src/components/BridgeInbox.tsx` - show task type, sandbox, scope, evidence.
- Modify: `src/types/orchestrator.ts` - TypeScript mirrors for task, graph, sandbox, kernel results.

---

### Task 1: Add MDT Types And Frontmatter Parsing

**Files:**
- Create: `src-tauri/src/mdt/mod.rs`
- Create: `src-tauri/src/mdt/types.rs`
- Modify: `src-tauri/src/parser/frontmatter.rs`
- Modify: `src-tauri/src/lib.rs` or `src-tauri/src/main.rs` module registration path if needed

- [ ] **Step 1: Write failing tests for MDT frontmatter**

Add tests in `src-tauri/src/parser/frontmatter.rs`:

```rust
#[test]
fn test_parse_mdt_frontmatter_fields() {
    let content = "---\nmdt_version: \"0.1.0\"\nid: \"node-1\"\ntitle: MDT Node\ntree:\n  parent: null\n  order: 3\n  path: [root, design]\n  depth: 1\narea: memory-design\nimportance: 4\nsummary: \"A node summary\"\nlinks:\n  - target: \"node-2\"\n    type: reference\nstorage:\n  tier: warm\n  pinned: false\n---\n# Body\n";
    let (fm, offset) = parse_frontmatter(content).unwrap();
    assert_eq!(fm.title.as_deref(), Some("MDT Node"));
    assert_eq!(fm.mdt.as_ref().unwrap().id.as_deref(), Some("node-1"));
    assert_eq!(fm.mdt.as_ref().unwrap().tree.as_ref().unwrap().order, 3);
    assert_eq!(fm.mdt.as_ref().unwrap().links[0].edge_type, "reference");
    assert_eq!(&content[offset..], "# Body\n");
}
```

- [ ] **Step 2: Run the targeted Rust test**

Run: `cargo test parser::frontmatter::tests::test_parse_mdt_frontmatter_fields`

Expected: FAIL because `Frontmatter` has no `mdt` field yet.

- [ ] **Step 3: Add MDT serde structs**

Create `src-tauri/src/mdt/types.rs` with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtMeta {
    #[serde(default)]
    pub mdt_version: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub tree: Option<MdtTree>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub importance: Option<u8>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub links: Vec<MdtLink>,
    #[serde(default)]
    pub storage: Option<MdtStorage>,
    #[serde(default)]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtTree {
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub order: i64,
    #[serde(default)]
    pub path: Vec<String>,
    #[serde(default)]
    pub depth: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtLink {
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtStorage {
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
}
```

Create `src-tauri/src/mdt/mod.rs`:

```rust
pub mod types;
```

- [ ] **Step 4: Extend `Frontmatter` while keeping legacy behavior**

In `src-tauri/src/parser/frontmatter.rs`, import `MdtMeta` and add flattened optional MDT fields:

```rust
use crate::mdt::types::MdtMeta;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
    #[serde(flatten)]
    pub mdt: Option<MdtMeta>,
}
```

If `serde(flatten)` with `Option<MdtMeta>` does not deserialize as expected, replace it with explicit optional fields on `Frontmatter` and add `impl Frontmatter { pub fn mdt_meta(&self) -> Option<MdtMeta> }`.

- [ ] **Step 5: Run parser tests**

Run: `cargo test parser::frontmatter`

Expected: PASS for legacy tests and the new MDT test.

### Task 2: Implement MDT Indexer And Reader

**Files:**
- Create: `src-tauri/src/mdt/indexer.rs`
- Create: `src-tauri/src/mdt/reader.rs`
- Modify: `src-tauri/src/mdt/mod.rs`
- Modify: `src-tauri/src/ai/commands.rs`

- [ ] **Step 1: Write indexer tests**

Create tests that build a temporary vault with three Markdown files:

```rust
#[test]
fn test_indexer_emits_nodes_and_typed_edges() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("root.md"), "---\nmdt_version: \"0.1.0\"\nid: root\ntitle: Root\ntree:\n  parent: null\n  order: 0\nlinks:\n  - target: child\n    type: parent\n---\n# Root\n").unwrap();
    std::fs::write(temp.path().join("child.md"), "---\nmdt_version: \"0.1.0\"\nid: child\ntitle: Child\ntree:\n  parent: root\n  order: 1\narea: dream\n---\n# Child\n").unwrap();
    let index = crate::mdt::indexer::index_vault(temp.path()).unwrap();
    assert_eq!(index.nodes.len(), 2);
    assert_eq!(index.edges.len(), 1);
    assert_eq!(index.edges[0].edge_type, "parent");
}
```

- [ ] **Step 2: Run the indexer test**

Run: `cargo test mdt::indexer::tests::test_indexer_emits_nodes_and_typed_edges`

Expected: FAIL because `index_vault` is not implemented.

- [ ] **Step 3: Implement index data structs and scan logic**

Add `MdtNodeIndex`, `MdtEdgeIndex`, and `MdtProjectIndex` to `mdt/types.rs`. Implement `index_vault(path)` to recursively read `.md` and `.mdt`, parse frontmatter, normalize `links`, derive backlinks only in memory, and skip generated `.dualtrack` directories.

- [ ] **Step 4: Implement Reader L0-L3**

`MdtReader::load_context(project_root, query, budget)` returns:

```rust
pub struct MdtContextBundle {
    pub query: String,
    pub remaining_budget: usize,
    pub items: Vec<MdtContextItem>,
}

pub struct MdtContextItem {
    pub node_id: String,
    pub level: MdtReadLevel,
    pub reason: String,
    pub content: String,
}
```

Ranking must include title/tags/area/summary match, same branch, direct links, importance, and storage tier. L3 is only used for the strongest candidates.

- [ ] **Step 5: Expose IPC commands**

Add commands in `src-tauri/src/ai/commands.rs`:

```rust
mdt_validate(project_root: String) -> Result<MdtValidationReport, String>
mdt_index(project_root: String) -> Result<MdtProjectIndex, String>
mdt_read(project_root: String, query: String, token_budget: usize) -> Result<MdtContextBundle, String>
```

- [ ] **Step 6: Run MDT tests**

Run: `cargo test mdt`

Expected: PASS.

### Task 3: Extend Graph Data With Typed Edges And Dream Projection

**Files:**
- Modify: `src-tauri/src/graph/link_graph.rs`
- Modify: `src-tauri/src/graph/commands.rs`
- Modify: `src-tauri/src/ai/dream_engine.rs`
- Modify: `src-tauri/src/fs/commands.rs`

- [ ] **Step 1: Write graph tests for typed edges**

Add a test in `src-tauri/src/graph/link_graph.rs`:

```rust
#[test]
fn test_typed_edges_are_exported() {
    let mut graph = LinkGraph::new();
    graph.add_typed_link("a.md", "b.md", GraphEdgeType::Reference, "wikilink", 1.0);
    let data = graph.to_frontend_json();
    assert_eq!(data.edges[0].edge_type, GraphEdgeType::Reference);
    assert_eq!(data.edges[0].origin, "wikilink");
}
```

- [ ] **Step 2: Run the graph test**

Run: `cargo test graph::link_graph::tests::test_typed_edges_are_exported`

Expected: FAIL because `GraphEdgeType` and `add_typed_link` do not exist.

- [ ] **Step 3: Update graph model**

Replace `HashSet<String>` edge storage with a keyed edge map:

```rust
edges: HashMap<(String, String, GraphEdgeType), GraphEdgeMeta>
```

Keep `add_link(from, to)` as a compatibility wrapper using `GraphEdgeType::Reference`.

- [ ] **Step 4: Map Dream connections into graph**

Add a method in `dream_engine.rs`:

```rust
pub fn export_graph_edges(&self) -> Vec<GraphEdge>
```

Map `ConnectionType::Semantic` to `semantic`, `Temporal` to `temporal`, `Reference` to `reference`, and `Bridge` to `bridge`.

- [ ] **Step 5: Rebuild graph using wikilinks, MDT links, and Dream edges**

In `fs/commands.rs`, keep existing wikilink extraction, add typed MDT links when frontmatter provides them, and merge Dream edges when a DreamEngine database is available.

- [ ] **Step 6: Run graph tests**

Run: `cargo test graph`

Expected: PASS.

### Task 4: Render Dream-Aware GraphView

**Files:**
- Modify: `src/components/GraphView.tsx`
- Modify: `src/types/orchestrator.ts`
- Add or modify component tests under `src/components/__tests__/`

- [ ] **Step 1: Add TypeScript graph types**

Extend graph edge/node types:

```ts
type GraphEdgeType =
  | "parent"
  | "reference"
  | "related"
  | "source"
  | "sequence"
  | "semantic"
  | "temporal"
  | "bridge";

interface GraphEdge {
  from: string;
  to: string;
  edge_type: GraphEdgeType;
  origin: string;
  confidence: number;
  weight?: number;
  memory_region?: string;
}
```

- [ ] **Step 2: Add line-style rendering**

In the canvas edge render loop, draw each edge separately and apply:

```ts
const styleForEdge = (edgeType: GraphEdgeType) => {
  switch (edgeType) {
    case "parent": return { dash: [], width: 1.8, alpha: 0.85 };
    case "reference": return { dash: [6, 4], width: 1.2, alpha: 0.7 };
    case "related": return { dash: [2, 5], width: 1, alpha: 0.55 };
    case "source": return { dash: [8, 4], width: 1.4, alpha: 0.8 };
    case "sequence": return { dash: [10, 4], width: 1.2, alpha: 0.75 };
    case "semantic": return { dash: [], width: 0.8, alpha: 0.32 };
    case "temporal": return { dash: [3, 3], width: 0.9, alpha: 0.42 };
    case "bridge": return { dash: [], width: 2.2, alpha: 0.9 };
  }
};
```

- [ ] **Step 3: Add graph filters**

Add a compact legend with toggle buttons for `parent/reference/related/source/sequence/semantic/temporal/bridge`. Disabled types are omitted from `links`.

- [ ] **Step 4: Verify frontend tests**

Run: `bun test src/components/__tests__`

Expected: PASS.

### Task 5: Add Typed Task Intake And Bridge Metadata

**Files:**
- Create: `src-tauri/src/ai/task_intent.rs`
- Modify: `src-tauri/src/ai/agent_scheduler.rs`
- Modify: `src-tauri/src/ai/commands.rs`
- Modify: `src-tauri/src/bridge/proposal.rs`
- Modify: `src/components/CliBar.tsx`
- Modify: `src/components/CommandCardPanel.tsx`
- Modify: `src/components/BridgeInbox.tsx`

- [ ] **Step 1: Add task intent tests**

Write Rust tests that confirm `research`, `dream`, and `mdt_index` map to different sandbox defaults and Bridge risks.

- [ ] **Step 2: Implement `TaskIntentType` and defaults**

Create enum values: `Research`, `Summarize`, `Verify`, `Dream`, `MdtIndex`, `MdtRead`, `MdtPack`, `WriteProposal`, `ExternalImport`, `CodeAssist`.

- [ ] **Step 3: Update task submission**

Change `submit_task` to accept `{ task_type, content, params }`. CLI strings can still infer a type, but UI submissions must pass a type.

- [ ] **Step 4: Update Bridge proposal metadata**

Add `task_type`, `sandbox_summary`, `expected_output`, and `risk_reason` to Bridge proposals.

- [ ] **Step 5: Run task and bridge tests**

Run: `cargo test ai::task_intent bridge`

Expected: PASS.

### Task 6: Enforce Subagent Sandbox Policy

**Files:**
- Create: `src-tauri/src/ai/sandbox.rs`
- Modify: `src-tauri/src/ai/tool_registry.rs`
- Modify: `src-tauri/src/ai/subagent.rs`
- Modify: `src-tauri/src/ai/agent_scheduler.rs`
- Modify: `src-tauri/src/harness/orchestrator.rs`

- [ ] **Step 1: Write sandbox tests**

Add tests:

```rust
#[test]
fn test_sandbox_blocks_unlisted_tool() {
    let policy = SandboxPolicy::read_only_research();
    assert!(!policy.allows_tool("write_note"));
    assert!(policy.allows_tool("vector_search"));
}

#[test]
fn test_sandbox_blocks_write_outside_root() {
    let policy = SandboxPolicy::with_write_roots(vec![PathBuf::from("vault/.dualtrack/ghosts")]);
    assert!(!policy.allows_write(Path::new("vault/source.md")));
    assert!(policy.allows_write(Path::new("vault/.dualtrack/ghosts/new.md")));
}
```

- [ ] **Step 2: Run sandbox tests**

Run: `cargo test ai::sandbox`

Expected: FAIL before implementation.

- [ ] **Step 3: Implement policy checks**

`ToolRegistry::execute` must receive a `SandboxPolicy` and reject any tool outside the allowlist. Subagent file operations must check `allows_read` and `allows_write`.

- [ ] **Step 4: Remove direct write capability from Orchestrator paths**

Audit orchestrator-created tasks so they carry materials and instructions, not direct file write handles. All writes must produce Ghost, generated index files, archive files, or Bridge proposals.

- [ ] **Step 5: Run AI tests**

Run: `cargo test ai`

Expected: PASS.

### Task 7: Add Dream-Aware Regression Metrics

**Files:**
- Create: `src-tauri/src/harness/regression.rs`
- Modify: `src-tauri/src/harness/orchestrator.rs`
- Modify: `src-tauri/src/ai/dream_engine.rs`
- Modify: `src/components/OrchestratorPanel.tsx`

- [ ] **Step 1: Write regression tests**

Add tests for reason codes: `NoveltyPlateau`, `EvidencePlateau`, `DreamCoverageReached`, `ContradictionRiskHigh`, `ToolLoop`.

- [ ] **Step 2: Implement `EpochAuditResult`**

Add:

```rust
pub enum EpochEndReason {
    NoveltyPlateau,
    EvidencePlateau,
    DreamCoverageReached,
    ContradictionRiskHigh,
    ToolLoop,
    BudgetExhausted,
    HumanInterrupted,
}
```

- [ ] **Step 3: Feed Dream metrics into audit**

Expose Dream community coverage and salience shifts from DreamEngine, then pass them into `Orchestrator::audit_epoch`.

- [ ] **Step 4: Update UI**

Show the reason code and metric deltas in `OrchestratorPanel`.

- [ ] **Step 5: Run harness tests**

Run: `cargo test harness`

Expected: PASS.

### Task 8: Reposition LeanKernel As PropositionKernel

**Files:**
- Create: `src-tauri/src/harness/proposition_kernel.rs`
- Modify: `src-tauri/src/harness/lean_kernel.rs`
- Modify: `src-tauri/src/harness/lean_translator.rs`
- Modify: `src-tauri/src/harness/scientist.rs`
- Modify: `src/components/BridgeInbox.tsx`

- [ ] **Step 1: Add compatibility wrapper**

Create `proposition_kernel.rs` that re-exports existing graph types and calls `HybridLeanKernel::verify` internally.

- [ ] **Step 2: Add severity and repair hints**

Extend `Violation` output with `severity: "info" | "warning" | "error"` and `repair_hint: Option<String>`.

- [ ] **Step 3: Update Scientist output**

ScientistResult must include `claims`, `sources`, `evidence_chain`, `verification`, and `kernel_name: "PropositionKernel"`.

- [ ] **Step 4: Update UI copy**

Bridge and dashboard should say "Proposition consistency" or "LeanLite" rather than implying a real theorem prover.

- [ ] **Step 5: Run harness tests**

Run: `cargo test harness::proposition_kernel harness::lean_kernel`

Expected: PASS.

### Task 9: End-To-End Release Verification

**Files:**
- Modify: `package.json` scripts if a missing verification command is needed.
- Modify: `src/components/__tests__/` for graph/task UI coverage.
- Modify: `src-tauri/src/**` tests added in earlier tasks.

- [ ] **Step 1: Run Rust tests**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 2: Run frontend tests**

Run: `bun test`

Expected: PASS.

- [ ] **Step 3: Run typecheck**

Run: `bun x tsc -p tsconfig.json --noEmit`

Expected: PASS.

- [ ] **Step 4: Run Tauri build check**

Run: `cargo check`

Expected: PASS.

- [ ] **Step 5: Manual E2E acceptance**

Open a vault and verify:

- Old wikilinks still appear in GraphView.
- MDT index produces typed edges.
- Dream cycle adds bridge/semantic/temporal edges.
- Typed task submission shows sandbox and risk metadata.
- Orchestrator-created tracks do not write source notes directly.
- Scientist output appears as Bridge proposal with evidence and PropositionKernel violations.

