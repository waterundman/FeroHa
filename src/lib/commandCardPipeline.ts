import type { CommandCardDefinition } from "../types/command-card";
import type { ContextFragment } from "../types/context-fragment";
import { PromptTemplateEngine } from "./promptTemplate";
import { VariableResolver } from "./variableResolver";
import { useAppStore } from "../hooks/useAppStore";

// Pipeline Node Types
export type NodeType = "command" | "condition" | "parallel" | "start" | "end";

export interface ContextPort {
  input: Map<string, ContextFragment>;
  output: Map<string, ContextFragment>;
  dependencies: string[];
}

export interface PipelineNode {
  id: string;
  type: NodeType;
  card?: CommandCardDefinition;
  params?: Record<string, unknown>;
  condition?: string;
  position: { x: number; y: number };
  context?: ContextPort;
}

export interface PipelineEdge {
  id: string;
  source: string;
  target: string;
  label?: string;
}

export interface PipelineDefinition {
  id: string;
  name: string;
  description: string;
  nodes: PipelineNode[];
  edges: PipelineEdge[];
  variables: Record<string, unknown>;
  version: string;
  createdAt: string;
  updatedAt: string;
}

export type PipelineStatus = "idle" | "running" | "paused" | "completed" | "failed" | "cancelled";

export interface PipelineExecution {
  id: string;
  pipelineId: string;
  status: PipelineStatus;
  currentNodeId?: string;
  results: Map<string, unknown>;
  errors: Map<string, Error>;
  startedAt?: string;
  completedAt?: string;
  progress: number;
  contextTrail?: ContextFragment[];
}

export type ProgressCallback = (execution: PipelineExecution) => void;
export type NodeCallback = (node: PipelineNode, result: unknown) => void;

// Pipeline Engine
export class CommandCardPipelineEngine {
  private templateEngine: PromptTemplateEngine;
  private variableResolver: VariableResolver;
  private executions: Map<string, PipelineExecution> = new Map();
  private cancelTokens: Map<string, boolean> = new Map();

  constructor() {
    this.templateEngine = new PromptTemplateEngine();
    this.variableResolver = new VariableResolver();
  }

  // Execute a pipeline
  async execute(
    pipeline: PipelineDefinition,
    initialVariables: Record<string, unknown> = {},
    onProgress?: ProgressCallback,
    onNodeComplete?: NodeCallback
  ): Promise<PipelineExecution> {
    const executionId = `exec_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    
    const execution: PipelineExecution = {
      id: executionId,
      pipelineId: pipeline.id,
      status: "running",
      results: new Map(),
      errors: new Map(),
      startedAt: new Date().toISOString(),
      progress: 0,
    };

    this.executions.set(executionId, execution);
    this.cancelTokens.set(executionId, false);

    try {
      // Merge initial variables with pipeline variables
      const variables = { ...pipeline.variables, ...initialVariables };
      this.variableResolver.updateContext({ userInputs: variables });

      // Find start node
      const startNode = pipeline.nodes.find(n => n.type === "start");
      if (!startNode) {
        throw new Error("Pipeline has no start node");
      }

      // Execute pipeline
      await this.executeNode(pipeline, startNode, execution, onProgress, onNodeComplete);

      // Mark as completed
      execution.status = "completed";
      execution.completedAt = new Date().toISOString();
      execution.progress = 100;

    } catch (error) {
      execution.status = "failed";
      execution.completedAt = new Date().toISOString();
      throw error;

    } finally {
      this.cancelTokens.delete(executionId);
      onProgress?.(execution);
    }

    return execution;
  }

  // Cancel a running execution
  cancel(executionId: string): boolean {
    const token = this.cancelTokens.get(executionId);
    if (token !== undefined) {
      this.cancelTokens.set(executionId, true);
      const execution = this.executions.get(executionId);
      if (execution) {
        execution.status = "cancelled";
      }
      return true;
    }
    return false;
  }

  // Get execution status
  getExecution(executionId: string): PipelineExecution | undefined {
    return this.executions.get(executionId);
  }

  // Execute a single node
  private async executeNode(
    pipeline: PipelineDefinition,
    node: PipelineNode,
    execution: PipelineExecution,
    onProgress?: ProgressCallback,
    onNodeComplete?: NodeCallback
  ): Promise<void> {
    // Check for cancellation
    if (this.cancelTokens.get(execution.id)) {
      throw new Error("Pipeline execution cancelled");
    }

    // Update current node
    execution.currentNodeId = node.id;
    onProgress?.(execution);

    // Execute based on node type
    switch (node.type) {
      case "start":
        await this.executeNext(pipeline, node, execution, onProgress, onNodeComplete);
        break;

      case "end":
        // Pipeline complete
        break;

      case "command":
        await this.executeCommandNode(pipeline, node, execution, onProgress, onNodeComplete);
        break;

      case "condition":
        await this.executeConditionNode(pipeline, node, execution, onProgress, onNodeComplete);
        break;

      case "parallel":
        await this.executeParallelNode(pipeline, node, execution, onProgress, onNodeComplete);
        break;
    }
  }

  // Execute a command node
  private async executeCommandNode(
    pipeline: PipelineDefinition,
    node: PipelineNode,
    execution: PipelineExecution,
    onProgress?: ProgressCallback,
    onNodeComplete?: NodeCallback
  ): Promise<void> {
    if (!node.card) {
      throw new Error(`Command node ${node.id} has no card definition`);
    }

    // Collect input context from node's context port
    if (node.context) {
      const contextInputs: Record<string, unknown> = {};
      for (const [key, fragment] of node.context.input) {
        contextInputs[key] = fragment.value;
      }
      if (Object.keys(contextInputs).length > 0) {
        this.variableResolver.updateContext({ userInputs: contextInputs });
      }
    }

    try {
      // Resolve variables in params
      const resolvedParams: Record<string, unknown> = {};
      if (node.params) {
        for (const [key, value] of Object.entries(node.params)) {
          if (typeof value === "string" && value.includes("{{")) {
            resolvedParams[key] = this.templateEngine.render(value, this.variableResolver.getContext());
          } else {
            resolvedParams[key] = value;
          }
        }
      }

      // Execute command via Tauri IPC or local simulation
      const result = await this.simulateCommand(node.card, resolvedParams);

      // Store result
      execution.results.set(node.id, result);

      // Update variables with result
      const currentContext = this.variableResolver.getContext();
      this.variableResolver.updateContext({
        userInputs: {
          ...currentContext.userInputs,
          [`result_${node.id}`]: result,
        },
      });

      // Record output context and append to context trail
      if (node.context) {
        const outputFragments = this.variableResolver.toFragments();
        for (const fragment of outputFragments) {
          node.context.output.set(fragment.key, fragment);
        }
        if (execution.contextTrail) {
          execution.contextTrail.push(...outputFragments);
        }
      }

      // Notify completion
      onNodeComplete?.(node, result);

      // Execute next node
      await this.executeNext(pipeline, node, execution, onProgress, onNodeComplete);

    } catch (error) {
      execution.errors.set(node.id, error as Error);
      throw error;
    }
  }

  // Execute a condition node
  private async executeConditionNode(
    pipeline: PipelineDefinition,
    node: PipelineNode,
    execution: PipelineExecution,
    onProgress?: ProgressCallback,
    onNodeComplete?: NodeCallback
  ): Promise<void> {
    if (!node.condition) {
      throw new Error(`Condition node ${node.id} has no condition`);
    }

    try {
      // Evaluate condition
      const result = this.evaluateCondition(node.condition);
      execution.results.set(node.id, result);

      // Find next node based on condition result
      const nextEdges = pipeline.edges.filter(e => e.source === node.id);
      const nextEdge = result
        ? nextEdges.find(e => e.label === "true" || !e.label)
        : nextEdges.find(e => e.label === "false");

      if (nextEdge) {
        const nextNode = pipeline.nodes.find(n => n.id === nextEdge.target);
        if (nextNode) {
          await this.executeNode(pipeline, nextNode, execution, onProgress, onNodeComplete);
        }
      }

    } catch (error) {
      execution.errors.set(node.id, error as Error);
      throw error;
    }
  }

  // Execute a parallel node
  private async executeParallelNode(
    pipeline: PipelineDefinition,
    node: PipelineNode,
    execution: PipelineExecution,
    onProgress?: ProgressCallback,
    onNodeComplete?: NodeCallback
  ): Promise<void> {
    // Find all outgoing edges
    const nextEdges = pipeline.edges.filter(e => e.source === node.id);
    
    // Execute all branches in parallel
    const promises = nextEdges.map(async (edge) => {
      const nextNode = pipeline.nodes.find(n => n.id === edge.target);
      if (nextNode) {
        await this.executeNode(pipeline, nextNode, execution, onProgress, onNodeComplete);
      }
    });

    await Promise.all(promises);
  }

  // Execute the next node in sequence
  private async executeNext(
    pipeline: PipelineDefinition,
    currentNode: PipelineNode,
    execution: PipelineExecution,
    onProgress?: ProgressCallback,
    onNodeComplete?: NodeCallback
  ): Promise<void> {
    const nextEdge = pipeline.edges.find(e => e.source === currentNode.id);
    if (nextEdge) {
      const nextNode = pipeline.nodes.find(n => n.id === nextEdge.target);
      if (nextNode) {
        await this.executeNode(pipeline, nextNode, execution, onProgress, onNodeComplete);
      }
    }
  }

  // Simulate command execution (bridged to real Tauri IPC)
  private async simulateCommand(
    card: CommandCardDefinition,
    params: Record<string, unknown>
  ): Promise<unknown> {
    const intent = card.meta.description || card.meta.name;
    const renderedPrompt = this.templateEngine.render(card.prompt.template, {
      userInputs: params,
    });
    const content = renderedPrompt.content;

    const stringParams: Record<string, string> = {};
    for (const [key, value] of Object.entries(params)) {
      stringParams[key] = String(value);
    }

    let context_note: string | null = null;
    try {
      context_note = useAppStore.getState().currentNote?.path ?? null;
    } catch {
      // store not accessible (e.g. SSR)
    }

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      return await invoke("dispatch_agent_task", {
        payload: {
          intent,
          content,
          card_id: card.meta.id,
          card_type: card.meta.type,
          prompt: renderedPrompt.content,
          params: stringParams,
          context_note,
          timestamp: Date.now(),
        },
      });
    } catch {
      // Fallback: if Tauri invoke fails (e.g. browser without Tauri), return mock result
      return {
        cardId: card.meta.id,
        params,
        result: `[local] Result for ${card.meta.name}`,
        timestamp: new Date().toISOString(),
      };
    }
  }

  // Evaluate a condition expression
  private evaluateCondition(condition: string): boolean {
    try {
      // Evaluate condition expression using variable context
      const context = this.variableResolver.getContext();
      const fn = new Function(...Object.keys(context), `return ${condition}`);
      return fn(...Object.values(context));
    } catch {
      return false;
    }
  }
}

// Pipeline Manager (CRUD operations)
export class PipelineManager {
  private pipelines: Map<string, PipelineDefinition> = new Map();

  constructor() {
    this.load();
  }

  private save() {
    try {
      const data = JSON.stringify(Array.from(this.pipelines.entries()));
      localStorage.setItem("feroha-pipelines", data);
    } catch {
      // silent fail
    }
  }

  private load() {
    try {
      const raw = localStorage.getItem("feroha-pipelines");
      if (raw) {
        const entries: [string, PipelineDefinition][] = JSON.parse(raw);
        this.pipelines = new Map(entries);
      }
    } catch {
      // silent fail
    }
  }

  // Create a new pipeline
  create(name: string, description: string): PipelineDefinition {
    const pipeline: PipelineDefinition = {
      id: `pipeline_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      name,
      description,
      nodes: [
        { id: "start", type: "start", position: { x: 100, y: 200 } },
        { id: "end", type: "end", position: { x: 700, y: 200 } },
      ],
      edges: [],
      variables: {},
      version: "1.0.0",
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };

    this.pipelines.set(pipeline.id, pipeline);
    this.save();
    return pipeline;
  }

  // Get a pipeline by ID
  get(id: string): PipelineDefinition | undefined {
    return this.pipelines.get(id);
  }

  // Get all pipelines
  getAll(): PipelineDefinition[] {
    return Array.from(this.pipelines.values());
  }

  // Update a pipeline
  update(id: string, updates: Partial<PipelineDefinition>): PipelineDefinition | undefined {
    const pipeline = this.pipelines.get(id);
    if (pipeline) {
      const updated = { ...pipeline, ...updates, updatedAt: new Date().toISOString() };
      this.pipelines.set(id, updated);
      this.save();
      return updated;
    }
    return undefined;
  }

  // Delete a pipeline
  delete(id: string): boolean {
    const result = this.pipelines.delete(id);
    if (result) this.save();
    return result;
  }

  // Add a node to a pipeline
  addNode(pipelineId: string, node: Omit<PipelineNode, "id">): PipelineNode | undefined {
    const pipeline = this.pipelines.get(pipelineId);
    if (pipeline) {
      const newNode: PipelineNode = {
        ...node,
        id: `node_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      };
      pipeline.nodes.push(newNode);
      pipeline.updatedAt = new Date().toISOString();
      this.save();
      return newNode;
    }
    return undefined;
  }

  // Remove a node from a pipeline
  removeNode(pipelineId: string, nodeId: string): boolean {
    const pipeline = this.pipelines.get(pipelineId);
    if (pipeline) {
      pipeline.nodes = pipeline.nodes.filter(n => n.id !== nodeId);
      pipeline.edges = pipeline.edges.filter(e => e.source !== nodeId && e.target !== nodeId);
      pipeline.updatedAt = new Date().toISOString();
      this.save();
      return true;
    }
    return false;
  }

  // Add an edge to a pipeline
  addEdge(pipelineId: string, edge: Omit<PipelineEdge, "id">): PipelineEdge | undefined {
    const pipeline = this.pipelines.get(pipelineId);
    if (pipeline) {
      const newEdge: PipelineEdge = {
        ...edge,
        id: `edge_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      };
      pipeline.edges.push(newEdge);
      pipeline.updatedAt = new Date().toISOString();
      this.save();
      return newEdge;
    }
    return undefined;
  }

  // Remove an edge from a pipeline
  removeEdge(pipelineId: string, edgeId: string): boolean {
    const pipeline = this.pipelines.get(pipelineId);
    if (pipeline) {
      pipeline.edges = pipeline.edges.filter(e => e.id !== edgeId);
      pipeline.updatedAt = new Date().toISOString();
      this.save();
      return true;
    }
    return false;
  }

  // Export a pipeline to JSON
  export(id: string): string | undefined {
    const pipeline = this.pipelines.get(id);
    if (pipeline) {
      return JSON.stringify(pipeline, null, 2);
    }
    return undefined;
  }

  // Import a pipeline from JSON
  import(json: string): PipelineDefinition | undefined {
    try {
      const pipeline = JSON.parse(json) as PipelineDefinition;
      this.pipelines.set(pipeline.id, pipeline);
      this.save();
      return pipeline;
    } catch {
      return undefined;
    }
  }
}

// Singleton instances
export const pipelineEngine = new CommandCardPipelineEngine();
export const pipelineManager = new PipelineManager();
