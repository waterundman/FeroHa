/**
 * Command Card Registry
 * FeroHa - Dual-Track AI Note IDE
 * Version: 2.1.8
 */

import {
  type CommandCardDefinition,
  type CommandCategory,
  type CommandType,
  type CommandCardFilter,
  type CommandCardExport,
  type LegacyCommandCardDefinition,
  type ParamDefinition,
  validateCommandCard,
  migrateLegacyCard,
} from "../types/command-card";

// ============================================================================
// Built-in Command Cards
// ============================================================================

/** 创建内置指令卡定义 */
function createBuiltinCard(
  id: string,
  type: CommandType,
  category: CommandCategory,
  name: string,
  description: string,
  icon: string,
  template: string,
  params: ParamDefinition[],
  tags: string[],
  priority: number = 0
): CommandCardDefinition {
  return {
    meta: {
      id,
      name,
      description,
      icon,
      category,
      type,
      version: "1.0.0",
      tags,
      isCustom: false,
      createdAt: new Date().toISOString(),
    },
    prompt: {
      template,
      variables: params.map((p) => ({
        name: p.templateVar || p.name,
        type: p.type,
        defaultValue: p.defaultValue,
        description: p.description,
        required: p.required,
      })),
    },
    params,
    priority,
  };
}

/** 内置指令卡列表 */
const BUILTIN_CARDS: CommandCardDefinition[] = [
  // Content Operations
  createBuiltinCard(
    "search",
    "search",
    "content",
    "Search",
    "Search notes by keyword or semantic similarity",
    "Search",
    "Search for notes related to: {{query}}",
    [
      { name: "query", label: "Query", type: "string", required: true, placeholder: "Enter search query...", templateVar: "query" },
      { name: "top_k", label: "Top K", type: "number", defaultValue: 5, description: "Number of results", templateVar: "top_k" },
    ],
    ["search", "find", "query"],
    10
  ),

  createBuiltinCard(
    "summarize",
    "summarize",
    "content",
    "Summarize",
    "Generate a summary of selected notes",
    "Pencil",
    "Summarize the following content in {{style}} format: {{target}}",
    [
      { name: "target", label: "Target", type: "textarea", required: true, placeholder: "Content to summarize...", templateVar: "target" },
      { name: "style", label: "Style", type: "select", defaultValue: "bullet", options: [
        { value: "bullet", label: "Bullet Points" },
        { value: "paragraph", label: "Paragraph" },
        { value: "outline", label: "Outline" },
      ], templateVar: "style" },
    ],
    ["summary", "overview", "brief"],
    9
  ),

  createBuiltinCard(
    "rewrite",
    "rewrite",
    "content",
    "Rewrite",
    "Rewrite content with improved expression",
    "Pen",
    "Rewrite the following content in {{style}} style: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to rewrite...", templateVar: "content" },
      { name: "style", label: "Style", type: "select", defaultValue: "formal", options: [
        { value: "formal", label: "Formal" },
        { value: "casual", label: "Casual" },
        { value: "academic", label: "Academic" },
        { value: "creative", label: "Creative" },
      ], templateVar: "style" },
    ],
    ["rewrite", "rephrase", "improve"],
    8
  ),

  createBuiltinCard(
    "translate",
    "translate",
    "content",
    "Translate",
    "Translate content to another language",
    "Globe",
    "Translate the following content to {{targetLanguage}}: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to translate...", templateVar: "content" },
      { name: "targetLanguage", label: "Target Language", type: "select", defaultValue: "English", options: [
        { value: "English", label: "English" },
        { value: "Chinese", label: "中文" },
        { value: "Japanese", label: "日本語" },
        { value: "Korean", label: "한국어" },
        { value: "Spanish", label: "Español" },
        { value: "French", label: "Français" },
        { value: "German", label: "Deutsch" },
      ], templateVar: "targetLanguage" },
    ],
    ["translate", "language", "convert"],
    8
  ),

  createBuiltinCard(
    "expand",
    "expand",
    "content",
    "Expand",
    "Expand content with more details and examples",
    "BookOpen",
    "Expand the following content with more details: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to expand...", templateVar: "content" },
      { name: "depth", label: "Depth", type: "select", defaultValue: "standard", options: [
        { value: "brief", label: "Brief" },
        { value: "standard", label: "Standard" },
        { value: "detailed", label: "Detailed" },
      ], templateVar: "depth" },
    ],
    ["expand", "elaborate", "detail"],
    7
  ),

  createBuiltinCard(
    "simplify",
    "simplify",
    "content",
    "Simplify",
    "Simplify content for better understanding",
    "Scissors",
    "Simplify the following content: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to simplify...", templateVar: "content" },
      { name: "level", label: "Level", type: "select", defaultValue: "moderate", options: [
        { value: "basic", label: "Basic" },
        { value: "moderate", label: "Moderate" },
        { value: "advanced", label: "Advanced" },
      ], templateVar: "level" },
    ],
    ["simplify", "clarify", "easy"],
    7
  ),

  createBuiltinCard(
    "brainstorm",
    "brainstorm",
    "content",
    "Brainstorm",
    "Generate creative ideas around a topic",
    "Lightbulb",
    "Brainstorm {{count}} creative ideas about: {{topic}}",
    [
      { name: "topic", label: "Topic", type: "string", required: true, placeholder: "Enter topic...", templateVar: "topic" },
      { name: "count", label: "Count", type: "number", defaultValue: 10, validation: { min: 1, max: 50 }, templateVar: "count" },
    ],
    ["brainstorm", "ideas", "creative"],
    8
  ),

  createBuiltinCard(
    "outline",
    "outline",
    "content",
    "Outline",
    "Generate a detailed outline for content",
    "ClipboardList",
    "Generate a detailed outline for: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to outline...", templateVar: "content" },
      { name: "depth", label: "Depth", type: "number", defaultValue: 3, validation: { min: 1, max: 5 }, templateVar: "depth" },
    ],
    ["outline", "structure", "organize"],
    7
  ),

  // Analysis Operations
  createBuiltinCard(
    "organize",
    "organize",
    "analysis",
    "Organize",
    "Organize and structure notes",
    "FolderOpen",
    "Organize the following content using {{method}} method: {{target}}",
    [
      { name: "target", label: "Target", type: "textarea", required: true, placeholder: "Content to organize...", templateVar: "target" },
      { name: "method", label: "Method", type: "select", defaultValue: "auto", options: [
        { value: "auto", label: "Auto" },
        { value: "chronological", label: "Chronological" },
        { value: "hierarchical", label: "Hierarchical" },
        { value: "topical", label: "Topical" },
      ], templateVar: "method" },
    ],
    ["organize", "structure", "arrange"],
    8
  ),

  createBuiltinCard(
    "connect",
    "connect",
    "analysis",
    "Connect",
    "Find and create connections between notes",
    "Link",
    "Find connections between: {{source}} and {{target}}",
    [
      { name: "source", label: "Source", type: "textarea", required: true, placeholder: "Source content...", templateVar: "source" },
      { name: "target", label: "Target", type: "textarea", required: true, placeholder: "Target content...", templateVar: "target" },
    ],
    ["connect", "link", "relate"],
    8
  ),

  createBuiltinCard(
    "analyze",
    "analyze",
    "analysis",
    "Analyze",
    "分析文本结构、论证逻辑和关键词提取",
    "FileSearch",
    "Analyze the following content: {{content}}",
    [
      { name: "target", label: "Target", type: "string", required: false, placeholder: "Target note path...", templateVar: "target" },
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to analyze...", templateVar: "content" },
    ],
    ["analyze", "analysis", "keyword", "structure"],
    8
  ),

  createBuiltinCard(
    "compare",
    "compare",
    "analysis",
    "Compare",
    "Compare multiple concepts or notes",
    "Scale",
    "Compare the following concepts in {{format}} format: {{concepts}}",
    [
      { name: "concepts", label: "Concepts", type: "textarea", required: true, placeholder: "Enter concepts to compare...", templateVar: "concepts" },
      { name: "format", label: "Format", type: "select", defaultValue: "table", options: [
        { value: "table", label: "Table" },
        { value: "pros-cons", label: "Pros & Cons" },
        { value: "paragraph", label: "Paragraph" },
      ], templateVar: "format" },
    ],
    ["compare", "contrast", "analyze"],
    7
  ),

  createBuiltinCard(
    "question",
    "question",
    "analysis",
    "Question",
    "Generate deep thinking questions",
    "HelpCircle",
    "Generate {{count}} deep thinking questions about: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to question...", templateVar: "content" },
      { name: "count", label: "Count", type: "number", defaultValue: 5, validation: { min: 1, max: 20 }, templateVar: "count" },
    ],
    ["question", "think", "explore"],
    7
  ),

  createBuiltinCard(
    "suggest",
    "suggest",
    "analysis",
    "Suggest",
    "Provide improvement suggestions",
    "MessageCircle",
    "Provide improvement suggestions for: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to improve...", templateVar: "content" },
      { name: "focus", label: "Focus", type: "select", defaultValue: "general", options: [
        { value: "general", label: "General" },
        { value: "clarity", label: "Clarity" },
        { value: "structure", label: "Structure" },
        { value: "style", label: "Style" },
      ], templateVar: "focus" },
    ],
    ["suggest", "improve", "recommend"],
    7
  ),

  createBuiltinCard(
    "review",
    "review",
    "analysis",
    "Review",
    "Review content for issues and improvements",
    "ZoomIn",
    "Review the following content for {{criteria}}: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to review...", templateVar: "content" },
      { name: "criteria", label: "Criteria", type: "select", defaultValue: "quality", options: [
        { value: "quality", label: "Quality" },
        { value: "accuracy", label: "Accuracy" },
        { value: "completeness", label: "Completeness" },
        { value: "consistency", label: "Consistency" },
      ], templateVar: "criteria" },
    ],
    ["review", "check", "evaluate"],
    7
  ),

  createBuiltinCard(
    "research",
    "research",
    "analysis",
    "Research",
    "Deep research on a topic using AI",
    "Microscope",
    "Conduct deep research on: {{topic}}",
    [
      { name: "topic", label: "Topic", type: "string", required: true, placeholder: "Enter research topic...", templateVar: "topic" },
      { name: "depth", label: "Depth", type: "select", defaultValue: "standard", options: [
        { value: "quick", label: "Quick" },
        { value: "standard", label: "Standard" },
        { value: "comprehensive", label: "Comprehensive" },
      ], templateVar: "depth" },
    ],
    ["research", "investigate", "study"],
    9
  ),

  // Format Operations
  createBuiltinCard(
    "format",
    "format",
    "format",
    "Format",
    "Format content to a specific format",
    "Ruler",
    "Format the following content to {{format}}: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to format...", templateVar: "content" },
      { name: "format", label: "Format", type: "select", defaultValue: "markdown", options: [
        { value: "markdown", label: "Markdown" },
        { value: "html", label: "HTML" },
        { value: "plain", label: "Plain Text" },
        { value: "json", label: "JSON" },
      ], templateVar: "format" },
    ],
    ["format", "convert", "style"],
    6
  ),

  createBuiltinCard(
    "extract",
    "extract",
    "format",
    "Extract",
    "Extract key information from content",
    "Upload",
    "Extract {{type}} from: {{content}}",
    [
      { name: "content", label: "Content", type: "textarea", required: true, placeholder: "Content to extract from...", templateVar: "content" },
      { name: "type", label: "Type", type: "select", defaultValue: "keypoints", options: [
        { value: "keypoints", label: "Key Points" },
        { value: "entities", label: "Entities" },
        { value: "dates", label: "Dates" },
        { value: "links", label: "Links" },
        { value: "code", label: "Code Blocks" },
      ], templateVar: "type" },
    ],
    ["extract", "key", "important"],
    6
  ),

  createBuiltinCard(
    "visualize",
    "visualize",
    "format",
    "Visualize",
    "Create visualizations for data",
    "BarChart3",
    "Visualize the following data as {{chartType}} chart: {{data}}",
    [
      { name: "data", label: "Data", type: "textarea", required: true, placeholder: "Data to visualize...", templateVar: "data" },
      { name: "chartType", label: "Chart Type", type: "select", defaultValue: "bar", options: [
        { value: "bar", label: "Bar Chart" },
        { value: "line", label: "Line Chart" },
        { value: "pie", label: "Pie Chart" },
        { value: "scatter", label: "Scatter Plot" },
      ], templateVar: "chartType" },
    ],
    ["visualize", "chart", "graph"],
    6
  ),

  // System Operations
  createBuiltinCard(
    "dream",
    "dream",
    "system",
    "Dream",
    "Run memory consolidation (NREM/REM/Insight)",
    "Moon",
    "Run dream cycle in {{mode}} mode",
    [
      { name: "mode", label: "Mode", type: "select", defaultValue: "full", options: [
        { value: "full", label: "Full Cycle" },
        { value: "nrem", label: "NREM Only" },
        { value: "rem", label: "REM Only" },
        { value: "insight", label: "Insight Only" },
      ], templateVar: "mode" },
    ],
    ["dream", "consolidate", "memory"],
    5
  ),

  // Agent Operations
  createBuiltinCard(
    "deep-research",
    "deep-research",
    "agent",
    "Deep Research",
    "LLM-guided 4-stage deep research cycle",
    "Search",
    "/agent research {{question}}",
    [
      { name: "question", label: "Question", type: "string", required: true, placeholder: "Enter research question...", templateVar: "question" },
      { name: "depth", label: "Depth", type: "number", defaultValue: 3, validation: { min: 1, max: 5 }, templateVar: "depth" },
    ],
    ["agent", "research", "deep-research"],
    10
  ),

  createBuiltinCard(
    "dream-cycle",
    "dream",
    "agent",
    "Dream Cycle",
    "Trigger NREM->REM->Insight memory consolidation",
    "Brain",
    "/agent dream",
    [],
    ["agent", "dream", "memory", "consolidation"],
    9
  ),

  createBuiltinCard(
    "multi-search",
    "multi-search",
    "agent",
    "Multi Search",
    "Multi-source retrieval across local, web, arxiv, and semantic scholar",
    "Zap",
    "/agent search {{query}} --sources {{sources}}",
    [
      { name: "query", label: "Query", type: "string", required: true, placeholder: "Enter search query...", templateVar: "query" },
      { name: "sources", label: "Sources", type: "multiselect", defaultValue: ["local", "web"], options: [
        { value: "local", label: "Local" },
        { value: "web", label: "Web" },
        { value: "arxiv", label: "arXiv" },
        { value: "semantic-scholar", label: "Semantic Scholar" },
      ], templateVar: "sources" },
    ],
    ["agent", "search", "multi-source", "retrieval"],
    9
  ),

  createBuiltinCard(
    "orchestrator-check",
    "orchestrator-check",
    "agent",
    "Orchestrator Check",
    "Show orchestrator agent states and health",
    "Activity",
    "/agent status",
    [],
    ["agent", "orchestrator", "status", "monitor"],
    8
  ),

  createBuiltinCard(
    "graph-analysis",
    "graph-analysis",
    "agent",
    "Graph Analysis",
    "Graph neighborhood deep dive on a concept",
    "GitBranch",
    "/agent deep-dive {{concept}}",
    [
      { name: "concept", label: "Concept", type: "string", required: true, placeholder: "Enter concept...", templateVar: "concept" },
      { name: "depth", label: "Depth", type: "number", defaultValue: 2, validation: { min: 1, max: 3 }, templateVar: "depth" },
    ],
    ["agent", "graph", "deep-dive", "analysis"],
    8
  ),
];

// ============================================================================
// Command Card Registry Class
// ============================================================================

export class CommandCardRegistry {
  private cards: Map<string, CommandCardDefinition> = new Map();
  private categoryIndex: Map<CommandCategory, Set<string>> = new Map();
  private tagIndex: Map<string, Set<string>> = new Map();

  constructor() {
    this.initializeBuiltinCards();
  }

  /** 初始化内置指令卡 */
  private initializeBuiltinCards(): void {
    for (const card of BUILTIN_CARDS) {
      this.register(card, false);
    }
  }

  /** 更新索引 */
  private updateIndexes(card: CommandCardDefinition): void {
    // 更新分类索引
    if (!this.categoryIndex.has(card.meta.category)) {
      this.categoryIndex.set(card.meta.category, new Set());
    }
    this.categoryIndex.get(card.meta.category)!.add(card.meta.id);

    // 更新标签索引
    for (const tag of card.meta.tags) {
      if (!this.tagIndex.has(tag)) {
        this.tagIndex.set(tag, new Set());
      }
      this.tagIndex.get(tag)!.add(card.meta.id);
    }
  }

  /** 从索引中移除 */
  private removeFromIndexes(card: CommandCardDefinition): void {
    // 从分类索引中移除
    this.categoryIndex.get(card.meta.category)?.delete(card.meta.id);

    // 从标签索引中移除
    for (const tag of card.meta.tags) {
      this.tagIndex.get(tag)?.delete(card.meta.id);
    }
  }

  // ============================================================================
  // CRUD Operations
  // ============================================================================

  /**
   * 注册指令卡
   * @param card 指令卡定义
   * @param validate 是否验证（默认true）
   * @returns 是否注册成功
   */
  register(card: CommandCardDefinition, validate: boolean = true): boolean {
    if (validate) {
      const errors = validateCommandCard(card);
      if (errors.length > 0) {
        console.error("Invalid command card:", errors);
        return false;
      }
    }

    // 如果已存在，先移除旧索引
    if (this.cards.has(card.meta.id)) {
      this.removeFromIndexes(this.cards.get(card.meta.id)!);
    }

    this.cards.set(card.meta.id, card);
    this.updateIndexes(card);
    return true;
  }

  /**
   * 获取指令卡
   * @param id 指令卡ID
   * @returns 指令卡定义或undefined
   */
  get(id: string): CommandCardDefinition | undefined {
    return this.cards.get(id);
  }

  /**
   * 更新指令卡
   * @param id 指令卡ID
   * @param updates 更新内容
   * @returns 是否更新成功
   */
  update(id: string, updates: Partial<CommandCardDefinition>): boolean {
    const existing = this.cards.get(id);
    if (!existing) {
      return false;
    }

    const updated: CommandCardDefinition = {
      ...existing,
      ...updates,
      meta: {
        ...existing.meta,
        ...updates.meta,
        id, // 确保ID不变
        updatedAt: new Date().toISOString(),
      },
    };

    const errors = validateCommandCard(updated);
    if (errors.length > 0) {
      console.error("Invalid command card update:", errors);
      return false;
    }

    this.removeFromIndexes(existing);
    this.cards.set(id, updated);
    this.updateIndexes(updated);
    return true;
  }

  /**
   * 删除指令卡
   * @param id 指令卡ID
   * @returns 是否删除成功
   */
  delete(id: string): boolean {
    const card = this.cards.get(id);
    if (!card) {
      return false;
    }

    // 内置指令卡不允许删除
    if (!card.meta.isCustom) {
      console.warn("Cannot delete built-in command card:", id);
      return false;
    }

    this.removeFromIndexes(card);
    this.cards.delete(id);
    return true;
  }

  /**
   * 检查指令卡是否存在
   * @param id 指令卡ID
   * @returns 是否存在
   */
  has(id: string): boolean {
    return this.cards.has(id);
  }

  // ============================================================================
  // Query Operations
  // ============================================================================

  /**
   * 获取所有指令卡
   * @param filter 过滤器
   * @returns 指令卡列表
   */
  getAll(filter?: CommandCardFilter): CommandCardDefinition[] {
    let cards = Array.from(this.cards.values());

    if (filter) {
      // 按分类过滤
      if (filter.category && filter.category !== "all") {
        cards = cards.filter((card) => card.meta.category === filter.category);
      }

      // 按标签过滤
      if (filter.tags && filter.tags.length > 0) {
        cards = cards.filter((card) =>
          filter.tags!.some((tag) => card.meta.tags.includes(tag))
        );
      }

      // 按搜索关键词过滤
      if (filter.query) {
        const query = filter.query.toLowerCase();
        cards = cards.filter(
          (card) =>
            card.meta.name.toLowerCase().includes(query) ||
            card.meta.description.toLowerCase().includes(query) ||
            card.meta.tags.some((tag) => tag.toLowerCase().includes(query))
        );
      }

      // 只显示自定义指令卡
      if (filter.customOnly) {
        cards = cards.filter((card) => card.meta.isCustom);
      }

      // 排序
      if (filter.sortBy) {
        cards.sort((a, b) => {
          let comparison = 0;
          switch (filter.sortBy) {
            case "name":
              comparison = a.meta.name.localeCompare(b.meta.name);
              break;
            case "priority":
              comparison = (a.priority || 0) - (b.priority || 0);
              break;
            case "category":
              comparison = a.meta.category.localeCompare(b.meta.category);
              break;
            case "createdAt":
              comparison = (a.meta.createdAt || "").localeCompare(b.meta.createdAt || "");
              break;
          }
          return filter.sortOrder === "desc" ? -comparison : comparison;
        });
      }
    }

    return cards;
  }

  /**
   * 按分类获取指令卡
   * @param category 分类
   * @returns 指令卡列表
   */
  getByCategory(category: CommandCategory): CommandCardDefinition[] {
    const ids = this.categoryIndex.get(category);
    if (!ids) return [];

    return Array.from(ids)
      .map((id) => this.cards.get(id)!)
      .filter(Boolean);
  }

  /**
   * 按标签获取指令卡
   * @param tag 标签
   * @returns 指令卡列表
   */
  getByTag(tag: string): CommandCardDefinition[] {
    const ids = this.tagIndex.get(tag);
    if (!ids) return [];

    return Array.from(ids)
      .map((id) => this.cards.get(id)!)
      .filter(Boolean);
  }

  /**
   * 搜索指令卡
   * @param query 搜索关键词
   * @returns 指令卡列表
   */
  search(query: string): CommandCardDefinition[] {
    return this.getAll({ query });
  }

  /**
   * 获取所有分类
   * @returns 分类列表
   */
  getCategories(): CommandCategory[] {
    return Array.from(this.categoryIndex.keys());
  }

  /**
   * 获取所有标签
   * @returns 标签列表
   */
  getTags(): string[] {
    return Array.from(this.tagIndex.keys());
  }

  /**
   * 获取指令卡数量
   * @returns 指令卡数量
   */
  get size(): number {
    return this.cards.size;
  }

  // ============================================================================
  // Import/Export Operations
  // ============================================================================

  /**
   * 导出所有指令卡
   * @param customOnly 是否只导出自定义指令卡
   * @returns 导出数据
   */
  export(customOnly: boolean = false): CommandCardExport {
    const cards = customOnly
      ? this.getAll({ customOnly: true })
      : this.getAll();

    return {
      version: "2.1.8",
      exportedAt: new Date().toISOString(),
      cards,
    };
  }

  /**
   * 导入指令卡
   * @param data 导入数据
   * @param overwrite 是否覆盖已存在的指令卡
   * @returns 导入结果
   */
  import(data: CommandCardExport, overwrite: boolean = false): { success: number; failed: number; errors: string[] } {
    const result = { success: 0, failed: 0, errors: [] as string[] };

    for (const card of data.cards) {
      if (this.has(card.meta.id) && !overwrite) {
        result.failed++;
        result.errors.push(`Card already exists: ${card.meta.id}`);
        continue;
      }

      if (this.register(card)) {
        result.success++;
      } else {
        result.failed++;
        result.errors.push(`Failed to register card: ${card.meta.id}`);
      }
    }

    return result;
  }

  /**
   * 导入旧版指令卡
   * @param legacyCards 旧版指令卡列表
   * @returns 导入结果
   */
  importLegacy(legacyCards: LegacyCommandCardDefinition[]): { success: number; failed: number } {
    let success = 0;
    let failed = 0;

    for (const legacy of legacyCards) {
      const card = migrateLegacyCard(legacy);
      if (this.register(card)) {
        success++;
      } else {
        failed++;
      }
    }

    return { success, failed };
  }

  // ============================================================================
  // Utility Operations
  // ============================================================================

  /**
   * 清空所有自定义指令卡
   */
  clearCustom(): void {
    const customCards = this.getAll({ customOnly: true });
    for (const card of customCards) {
      this.delete(card.meta.id);
    }
  }

  /**
   * 重置为内置指令卡
   */
  reset(): void {
    this.cards.clear();
    this.categoryIndex.clear();
    this.tagIndex.clear();
    this.initializeBuiltinCards();
  }

  /**
   * 克隆注册表
   * @returns 新的注册表实例
   */
  clone(): CommandCardRegistry {
    const newRegistry = new CommandCardRegistry();
    newRegistry.cards = new Map(this.cards);
    newRegistry.categoryIndex = new Map(this.categoryIndex);
    newRegistry.tagIndex = new Map(this.tagIndex);
    return newRegistry;
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

/** 全局指令卡注册表实例 */
export const commandCardRegistry = new CommandCardRegistry();
