# FeroHa (贝叶斯笔记)

**Dual-Track AI Note IDE** — A knowledge management system where humans and AI work in parallel tracks, with humans retaining final write authority.

Built with **Tauri 2 + React 18 + Rust**, FeroHa goes beyond traditional note-taking by integrating an AI memory architecture inspired by cognitive science: layered memory protocols, sleep-cycle memory consolidation, knowledge graph visualization, and sandboxed AI orchestration.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Frontend (React + Vite)               │
│  Editor · GraphView · DiffView · AgentDashboard · Bridge │
│  CodeMirror 6 · d3-force · Zustand · Resizable Panels   │
└────────────────────────┬────────────────────────────────┘
                         │ Tauri IPC
┌────────────────────────▼────────────────────────────────┐
│                   Backend (Rust / Tauri 2)               │
│                                                         │
│  ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌───────────┐  │
│  │  MDT    │ │  Dream   │ │   Graph   │ │  Bridge   │  │
│  │ Protocol│ │  Engine  │ │   Link    │ │  Proposal │  │
│  └────┬────┘ └────┬─────┘ └─────┬─────┘ └─────┬─────┘  │
│       │           │             │              │         │
│  ┌────▼───────────▼─────────────▼──────────────▼─────┐  │
│  │              Harness / Orchestrator                │  │
│  │  Scientist · PropositionKernel · Regression · RAG  │  │
│  └──────────────────────┬────────────────────────────┘  │
│                         │                               │
│  ┌──────────────────────▼────────────────────────────┐  │
│  │              AI Surface (Sandboxed)                │  │
│  │  LLM Router · Agent Scheduler · Tool Registry     │  │
│  │  Vector DB · Search Engine · Skill Manager         │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### Dual-Track Model

FeroHa separates **human editing** from **AI operations** into two parallel tracks:

| Track | Role | Write Access |
|-------|------|-------------|
| **Human Surface** | Direct note editing, vault browsing, bridge review | Full read/write on source notes |
| **AI Surface** | Research, indexing, dreaming, proposals | Read-only on source notes; writes go through Bridge Proposals |

The AI surface can never silently overwrite your notes. All AI-generated changes are presented as **Bridge Proposals** that require explicit human approval.

### MDT (Markdown Tree) Protocol

MDT extends standard Markdown files with structured metadata in YAML front matter, without breaking CommonMark compatibility:

```yaml
---
id: "abc123"
tree: "research/ai"
area: "machine-learning"
importance: 0.8
summary: "Notes on transformer attention mechanisms"
links: ["other-note-id"]
content_hash: "sha256..."
---
```

MDT provides **layered reading** (L0–L3) so the AI can progressively expand context within token budgets:

| Layer | Content |
|-------|---------|
| L0 | id, title, tree, area, tags, importance, storage |
| L1 | L0 + summary, links, headings |
| L2 | L1 + relevant paragraphs |
| L3 | Full text |

### Dream Engine

Inspired by human sleep cycles, the Dream Engine consolidates AI memories in three phases:

| Phase | Activity | Output |
|-------|----------|--------|
| **NREM** | Strengthen important connections, prune weak ones | Updated connection weights |
| **REM** | Create novel bridges between distant concepts | Cross-domain bridge edges |
| **Insight** | Discover communities and patterns | Community clusters, insight summaries |

Dream results are persisted to SQLite and visualized in GraphView with distinct edge types (bridge, semantic, temporal).

### Knowledge Graph

GraphView renders an interactive force-directed graph with **8 edge types**, each with distinct visual styling:

| Edge Type | Visual | Meaning |
|-----------|--------|---------|
| `parent` | Solid line | Tree hierarchy |
| `reference` | Dashed line | Wikilink reference |
| `related` | Dotted line | Weak relation |
| `source` | Dashed + arrow | Evidence source |
| `sequence` | Dashed + number | Ordered sequence |
| `semantic` | Thin + low opacity | Semantic similarity |
| `temporal` | Thin + time gradient | Temporal proximity |
| `bridge` | Highlighted curve | Dream REM bridge |

### Orchestrator & Sandbox

The Orchestrator coordinates AI tasks but **never directly writes to source notes**. Each subagent runs inside a `SandboxPolicy`:

```rust
pub struct SandboxPolicy {
    pub tool_allowlist: Vec<String>,    // which tools the agent can use
    pub read_roots: Vec<PathBuf>,       // allowed read directories
    pub write_roots: Vec<PathBuf>,      // allowed write directories
    pub network_policy: NetworkPolicy,  // Disabled | Allowed | AcademicOnly
    pub max_runtime_secs: u64,          // execution time limit
    pub requires_bridge: bool,          // must go through Bridge Proposal
}
```

Task types determine default sandbox policies:

| Task Type | Default Write | Bridge Risk |
|-----------|--------------|-------------|
| `research` | proposal only | medium |
| `summarize` | proposal only | low |
| `verify` | none | low |
| `dream` | proposal only | medium |
| `mdt_index` | generated files only | medium |
| `external_import` | proposal only | high |
| `code_assist` | proposal only | high |

### PropositionKernel

A structural consistency checker for knowledge claims (not a formal theorem prover):

- Detects circular dependencies in proposition graphs
- Identifies dangling references
- Flags direct contradictions
- Validates evidence chain completeness
- Assesses Scientist output readiness for Bridge Proposals

---

## Tech Stack

### Frontend

| Technology | Purpose |
|-----------|---------|
| React 18 | UI framework |
| Vite 5 | Build tool & dev server |
| TypeScript 5.5 | Type safety |
| CodeMirror 6 | Markdown editor with syntax highlighting |
| d3-force | Force-directed graph layout |
| Zustand | State management |
| react-resizable-panels | Split panel layout |
| Lucide React | Icon system |

### Backend (Rust)

| Crate | Purpose |
|-------|---------|
| Tauri 2 | Desktop app shell, IPC, window management |
| serde / serde_json / serde_yaml | Serialization |
| rusqlite | Embedded SQLite for AI state & vector DB |
| tantivy + tantivy-jieba | Full-text search with CJK support |
| reqwest | HTTP client for LLM API calls |
| tokio | Async runtime |
| pulldown-cmark | Markdown parsing |
| notify | File system watcher |
| chrono | Time handling |

### LLM Providers

| Provider | Models |
|----------|--------|
| Google Gemini | gemini-2.0-flash, gemini-1.5-pro |
| OpenAI | gpt-4o, gpt-4o-mini |
| Anthropic | claude-3.5-sonnet |
| DeepSeek | deepseek-chat |
| Ollama | Local models (llama3, mistral, etc.) |

The LLM Router supports **fallback cascading** — if the primary provider fails, it automatically retries with configured fallbacks.

---

## Project Structure

```
feroha/
├── src/                          # Frontend (React + TypeScript)
│   ├── components/               # UI components
│   │   ├── Editor.tsx            # CodeMirror 6 markdown editor
│   │   ├── GraphView.tsx         # d3-force knowledge graph
│   │   ├── DiffView.tsx          # AI proposal diff review
│   │   ├── AgentDashboard.tsx    # AI task monitoring
│   │   ├── BridgeInbox.tsx       # Human approval inbox
│   │   ├── OrchestratorPanel.tsx # Orchestrator status bar
│   │   ├── HumanTaskIntake.tsx   # Task submission interface
│   │   ├── InspirationCanvas.tsx # Freeform idea canvas
│   │   ├── CommandCard*.tsx      # Command card system
│   │   ├── VaultBrowser.tsx      # File tree navigator
│   │   ├── SettingsPanel.tsx     # Configuration UI
│   │   └── __tests__/            # Component tests
│   ├── hooks/
│   │   ├── useAppStore.ts        # Zustand global state
│   │   ├── useSettings.ts        # Settings persistence
│   │   └── useKeyboardShortcuts.ts
│   ├── lib/
│   │   ├── ipc.ts                # Tauri IPC bindings
│   │   ├── promptTemplate.ts     # Prompt template engine
│   │   └── variableResolver.ts   # Template variable resolution
│   ├── store/
│   │   ├── commandCardRegistry.ts
│   │   └── commandCardStore.ts
│   ├── types/                    # TypeScript type definitions
│   └── styles/                   # CSS themes
│
├── src-tauri/                    # Backend (Rust)
│   ├── src/
│   │   ├── ai/                   # AI surface modules
│   │   │   ├── agent_scheduler.rs
│   │   │   ├── api_client.rs     # LLM HTTP client
│   │   │   ├── dream_engine.rs   # NREM/REM/Insight memory
│   │   │   ├── dream_memory.rs   # Dream persistence
│   │   │   ├── llm_router.rs     # Multi-provider routing
│   │   │   ├── rag.rs            # Retrieval-augmented generation
│   │   │   ├── sandbox.rs        # Sandbox policies
│   │   │   ├── search_engine.rs  # Unified search
│   │   │   ├── subagent.rs       # Subagent management
│   │   │   ├── task_intent.rs    # Task type definitions
│   │   │   ├── tool_registry.rs  # Tool capability registry
│   │   │   ├── vectordb.rs       # SQLite vector store
│   │   │   └── workflow_*.rs     # Workflow runtime
│   │   ├── bridge/               # Bridge Proposal system
│   │   │   ├── proposal.rs       # Proposal creation & review
│   │   │   └── store.rs          # Proposal persistence
│   │   ├── diff/                 # Diff & merge engine
│   │   │   ├── ast_diff.rs       # AST-level diffing
│   │   │   ├── ghost_store.rs    # Ghost (draft) storage
│   │   │   └── merge_engine.rs   # Merge operations
│   │   ├── graph/                # Knowledge graph
│   │   │   ├── link_graph.rs     # In-memory bidirectional graph
│   │   │   ├── backlinks.rs      # Backlink computation
│   │   │   └── manifest.rs       # Graph manifest
│   │   ├── harness/              # AI orchestration layer
│   │   │   ├── orchestrator.rs   # Main orchestrator
│   │   │   ├── scientist.rs      # Knowledge extraction
│   │   │   ├── lean_kernel.rs    # Legacy Lean kernel
│   │   │   ├── proposition_kernel.rs # Consistency checker
│   │   │   ├── regression.rs     # Regression detection
│   │   │   ├── workflow.rs       # Workflow definitions
│   │   │   └── output_hook.rs    # Output processing
│   │   ├── mdt/                  # MDT protocol
│   │   │   ├── types.rs          # MDT data types
│   │   │   ├── reader.rs         # Layered reader (L0-L3)
│   │   │   ├── indexer.rs        # Vault indexer
│   │   │   └── archive.rs        # .mdtz pack/unpack
│   │   ├── jsonld/               # JSON-LD indexing
│   │   ├── parser/               # Markdown parser
│   │   │   ├── ast.rs            # AST representation
│   │   │   └── frontmatter.rs    # YAML front matter
│   │   ├── snapshot/             # Snapshot engine
│   │   ├── plugin/               # Plugin system
│   │   ├── fs/                   # File system operations
│   │   ├── ipc/                  # IPC protocol
│   │   └── cli/                  # CLI command parser
│   ├── capabilities/             # Tauri permission configs
│   └── icons/                    # App icons
│
├── e2e/                          # Playwright E2E tests
├── docs/                         # Documentation & specs
│   ├── superpowers/              # Design specs & audit reports
│   │   ├── specs/
│   │   ├── plans/
│   │   └── audits/
│   └── feroha-spec-v3.0.0-main.md
└── benches/                      # Performance benchmarks
```

---

## Getting Started

### Prerequisites

- **Node.js** >= 18
- **Rust** >= 1.70 (with `cargo`)
- **Tauri CLI** v2

### Install Dependencies

```bash
# Frontend
npm install

# Backend (Tauri will auto-resolve Rust deps on first build)
cd src-tauri && cargo check
```

### Development

```bash
# Start frontend dev server only (browser mode, no Tauri)
npm run dev:web

# Start full Tauri desktop app
npm run tauri dev
```

### Build

```bash
# Build frontend
npm run build

# Build desktop app
npm run tauri build
```

---

## Testing

### Unit Tests (Vitest)

```bash
npm run test           # Run once
npm run test:watch     # Watch mode
```

### E2E Tests (Playwright)

```bash
npm run e2e
```

### Rust Tests

```bash
cd src-tauri && cargo test
```

---

## Configuration

FeroHa stores settings in `~/.config/feroha/settings.json` (or platform equivalent). Key settings:

| Setting | Description | Default |
|---------|-------------|---------|
| `llm_provider` | Primary LLM provider | `gemini` |
| `llm_api_key` | API key for the provider | — |
| `llm_model` | Model identifier | `gemini-2.0-flash` |
| `embedding_provider` | Embedding model provider | `none` |
| `fallback_providers` | Fallback provider cascade | `[]` |
| `monthly_budget_usd` | API spending cap | `5.0` |
| `temperature` | LLM temperature | `0.7` |
| `theme` | UI theme | `dark` |

---

## Key Design Principles

1. **Human writes, AI proposes** — AI surface can read, index, research, and generate proposals, but never silently overwrite source notes.

2. **MDT is a metadata layer, not a new syntax** — Body text stays CommonMark; structural information lives in YAML front matter, manifests, and indexes.

3. **Compression is layered reading, not summary reconstruction** — The Reader uses L0–L3 to control read depth; AI cannot reconstruct unread original text from summaries.

4. **Dream connections must enter the graph protocol** — GraphView edges, colors, and opacity come from real memory chunks and connection types.

5. **Orchestrator is a coordinator, not an editor** — It plans, dispatches, audits, and summarizes, but does not hold direct file-writing tools.

6. **PropositionKernel checks structure, not truth** — It validates dependency integrity and conflict detection, not whether natural language claims are factually true.

---

## License

Private — All rights reserved.
