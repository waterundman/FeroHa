# FeroHa v2.14.8 Bridge Proposal Design

Date: 2026-05-28
Status: Draft for user review
Project: FeroHa
Theme: Dual-surface bridge loop

## Context

FeroHa is a local-first dual-track AI note IDE. Its core design is not "AI writes notes for the user"; it is a two-surface system where the human surface owns durable note writes and the AI surface can search, analyze, suggest, refine, verify, and export knowledge through explicit protocols.

The current v2.14.7 codebase already contains the important building blocks:

- Human surface: editor, vault browser, tabs, graph, diff review, command cards, pipeline editor, status bar.
- AI surface: AgentScheduler, ToolRegistry, TaskScheduler, DreamEngine, Scientist, OutputHook, TrustScore, GhostNote, research trace, TwoSurfaceProtocol.
- Existing bridge points: task approval, Diff/Ghost review, selection toolbar, TrustScore feedback, OutputHook output, AgentDashboard status.

The problem is that these bridge points are still scattered. A user can see tasks in one place, ghost diffs in another, dream insights in another, and tool availability in another. The AI surface has become more capable, but the human surface does not yet have one clear place to understand what the AI intends, why it believes the action is useful, what it would affect, and how to approve or reject it.

v2.14.8 should turn those scattered bridge points into one coherent proposal protocol.

## Goal

Create a Bridge Proposal layer so every important AI action that could affect the human knowledge base becomes a reviewable, traceable, and reversible suggestion card before it enters the human surface.

The user-facing goal is simple:

> AI can be proactive, but every important action arrives as an explainable proposal with evidence, impact, and explicit human controls.

## Non-Goals

- Do not grant the AI direct write access to human notes.
- Do not rewrite AgentScheduler, GhostStore, DreamEngine, Scientist, or ToolRegistry.
- Do not replace DiffView or AgentDashboard in this iteration.
- Do not build a remote sync or cloud collaboration workflow.
- Do not add a new LLM provider or embedding backend.

## Design Principles

1. Human writes remain authoritative.
   The Bridge Proposal layer may approve AI-generated changes, but final note mutation still goes through existing human-controlled commands, Diff/Ghost acceptance, or explicit apply actions.

2. AI intent must be visible.
   A proposal must state what the AI wants to do, why, where the evidence came from, and what will be affected.

3. TrustScore informs friction, not authority.
   Higher trust can change ordering and suggested defaults. It must not bypass human approval for high-impact actions.

4. Existing models remain the source of truth.
   BridgeProposal links to AgentTask, GhostNote, DreamInsight, ScientistResult, OutputHook payloads, and traces rather than duplicating all of their data.

5. The first version should be small and durable.
   v2.14.8 should establish the protocol, storage, inbox UI, and core source adapters. Later versions can add richer automation.

## Core Data Model

Add a Rust-side `BridgeProposal` model and a matching TypeScript type.

```ts
export interface BridgeProposal {
  id: string;
  source: "tool" | "scientist" | "dream" | "ghost" | "scheduler";
  source_ref: SourceRef;
  intent: string;
  summary: string;
  evidence: EvidenceRef[];
  impact: ImpactScope;
  risk: "low" | "medium" | "high";
  status: "pending" | "approved" | "rejected" | "applied" | "archived";
  actions: ProposalAction[];
  trust_snapshot: TrustScoreInfo;
  created_at: number;
  updated_at: number;
}

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

export interface ProposalAction {
  id: string;
  label: string;
  kind: "approve_task" | "open_diff" | "open_trace" | "apply_ghost" | "reject" | "archive";
  payload: Record<string, unknown>;
}
```

The Rust model should live in a new module:

- `src-tauri/src/bridge/mod.rs`
- `src-tauri/src/bridge/proposal.rs`
- `src-tauri/src/bridge/store.rs`
- `src-tauri/src/bridge/commands.rs`

The TypeScript type should live in:

- `src/types/bridge-proposal.ts`

## Storage

Bridge proposals should be stored under:

```text
{vault}/.dualtrack/bridge/proposals.json
```

The file stores a list of proposals. This matches the current local-first architecture and avoids adding a new database table before the proposal protocol stabilizes.

Persistence rules:

- `pending`, `approved`, `rejected`, and `applied` proposals are retained.
- `archived` proposals are retained but hidden by default.
- Proposal IDs are stable UUID strings.
- Updating status must append `updated_at`.
- Store writes should use the existing local filesystem safety style: create parent directory, serialize full JSON, then write.

## Source Adapters

BridgeProposal should be created by small adapters around existing systems.

### Agent Task Adapter

When a task is submitted or produced by a scheduler, create a proposal if the task is not already directly approved by an explicit user action.

Examples:

- `/agent research ...` submitted from CLI: proposal source `tool`, action `approve_task`.
- Dream auto-cycle from TaskScheduler: proposal source `scheduler`, action `approve_task`.
- CustomCard with `ghost_write`: proposal source `tool`, action `open_diff` or `apply_ghost`.

### Ghost/Diff Adapter

When a GhostNote is created, create or update a proposal linked to the ghost ID.

The proposal impact should include:

- target note path
- number of suggested blocks
- whether the ghost creates or modifies content
- conflict summary when available

Primary actions:

- open DiffView filtered to the ghost
- apply selected blocks through existing accept/reject paths
- reject proposal

### Scientist Adapter

When `Scientist::refine` completes, create a proposal if there are claims, verification results, or output payloads worth reviewing.

The proposal should summarize:

- number of claims
- overall confidence
- verification violations
- related source notes

Primary actions:

- open research trace
- archive
- export through OutputHook when applicable

### Dream Adapter

When DreamEngine creates insights, create one proposal per high-confidence insight or one grouped proposal for a dream cycle.

v2.14.8 should start with grouped proposals to avoid flooding the inbox.

Primary actions:

- open related notes
- archive insight
- create follow-up task proposal

## Backend Commands

Add IPC commands:

```rust
list_bridge_proposals(status_filter: Option<String>) -> Vec<BridgeProposal>
get_bridge_proposal(id: String) -> Option<BridgeProposal>
update_bridge_proposal_status(id: String, status: String) -> BridgeProposal
execute_bridge_action(id: String, action_id: String) -> BridgeProposalActionResult
```

`execute_bridge_action` should delegate to existing commands or internal methods:

- `approve_task` for task approval
- existing ghost accept/reject paths for diff actions
- trace lookup for `open_trace` metadata
- simple status updates for archive/reject

The command should return structured results, not free-form strings.

## Frontend Components

Add a new AI-mode panel:

- `src/components/BridgeInbox.tsx`
- `src/components/BridgeProposalCard.tsx`
- `src/components/BridgeProposalDetail.tsx`

Add a new tab icon in AI mode:

- title: `Bridge`
- icon: `Inbox` or `Workflow`

The inbox should group proposals by operational meaning:

1. Needs approval
2. Ready to review
3. Observations
4. Completed

Each card should show:

- source icon
- short intent
- status
- risk
- trust snapshot
- affected notes
- top evidence count
- primary action buttons

The detail view should show:

- full summary
- source reference
- evidence list
- impact scope
- available actions
- timestamps

## UX Flow

### Flow 1: Scheduled Dream

1. TaskScheduler submits a Dream task.
2. Bridge adapter creates a `scheduler` proposal.
3. Bridge Inbox shows "Dream cycle is ready to run".
4. User approves.
5. Existing task approval path runs.
6. When Dream finishes, insights create an observation proposal.

### Flow 2: Ghost Write

1. Agent uses `ghost_write`.
2. GhostStore creates a GhostNote.
3. Bridge adapter creates a `ghost` proposal.
4. Inbox shows affected note and suggested block count.
5. User opens DiffView from the card.
6. Accept/reject feedback updates both GhostNote and TrustScore.
7. Proposal status becomes `applied` or `rejected`.

### Flow 3: Scientist Result

1. DeepResearch completes.
2. Scientist refines claims and verification.
3. OutputHook may emit structured output.
4. Bridge adapter creates a `scientist` proposal.
5. Inbox shows claims, confidence, violations, and related notes.
6. User opens trace, archives, or exports.

## Risk Rules

Risk should be deterministic and conservative:

- `high`: modifies existing notes, exports data externally, or has verification violations.
- `medium`: creates ghost content, starts long-running research, or affects multiple notes.
- `low`: observation-only insight, local trace, or no note mutation.

High-risk proposals must remain pending until explicit human approval regardless of TrustScore.

## TrustScore Use

TrustScore should be captured as a snapshot when the proposal is created.

Use it for:

- sorting proposals inside the same group
- showing why an action is suggested
- marking low-trust proposals as requiring extra review

Do not use it for:

- automatic note writes
- suppressing high-risk approval
- deleting user data

## Error Handling

- If proposal storage cannot be read, return an empty list plus log a warning.
- If proposal storage cannot be written, return an IPC error and do not execute the requested action.
- If a proposal references a missing source object, keep the proposal visible and mark the evidence as unavailable.
- If an action partially succeeds, return `partial_success` with details and leave the proposal `pending` unless the terminal state is certain.
- If a duplicate source event arrives, update the existing proposal instead of creating duplicates when `source_ref` matches.

## Testing Plan

Backend tests:

- `BridgeProposalStore` creates, lists, updates, and persists proposals.
- Status transitions reject invalid status strings.
- Duplicate `source_ref` updates an existing proposal.
- Risk classification marks note mutation and external export as high risk.
- `execute_bridge_action` delegates task approval and archives proposals.

Frontend tests:

- BridgeInbox renders empty state.
- BridgeInbox groups proposals by status and action type.
- BridgeProposalCard shows intent, risk, trust snapshot, and affected notes.
- Approve/reject buttons call the expected IPC command.
- AI mode exposes the Bridge panel while human mode keeps the human surface uncluttered.

Verification commands:

```powershell
cargo test --lib
npm.cmd test
npm.cmd run build
```

## Acceptance Criteria

- A new Bridge panel is visible in AI mode.
- At least task, ghost, scientist, and dream proposal sources are modeled.
- Pending proposals can be listed, opened, approved, rejected, and archived.
- Approving a task proposal uses the existing AgentScheduler approval path.
- Ghost proposals can route the user into Diff review without writing directly to human notes.
- TrustScore appears on proposal cards but does not bypass high-risk approval.
- Existing tests continue to pass.

## Implementation Notes

This design intentionally creates a narrow bridge layer instead of merging existing panels. AgentDashboard remains the operational dashboard. DiffView remains the detailed merge surface. Bridge Inbox becomes the human-facing queue of AI intentions and outcomes.

The bridge layer should be useful even if some source adapters are initially shallow. The important v2.14.8 contract is that AI actions have one common review object and one common human-facing place to inspect them.
