/**
 * Command Card Type Definitions
 * FeroHa - Dual-Track AI Note IDE
 * Version: 2.1.8
 */

// ============================================================================
// Core Types
// ============================================================================

/** 指令类型枚举 */
export type CommandType =
  | "analyze"
  | "search"
  | "summarize"
  | "verify"
  | "organize"
  | "connect"
  | "dream"
  | "research"
  | "rewrite"
  | "translate"
  | "expand"
  | "simplify"
  | "brainstorm"
  | "outline"
  | "compare"
  | "question"
  | "suggest"
  | "review"
  | "format"
  | "extract"
  | "visualize"
  | "deep-research"
  | "multi-search"
  | "orchestrator-check"
  | "graph-analysis"
  | "custom";

/** 指令分类枚举 */
export type CommandCategory =
  | "content"
  | "analysis"
  | "format"
  | "system"
  | "agent";

/** 参数类型枚举 */
export type ParamType =
  | "string"
  | "number"
  | "boolean"
  | "select"
  | "multiselect"
  | "textarea"
  | "json";

/** 输出格式类型 */
export type OutputFormatType =
  | "text"
  | "markdown"
  | "json"
  | "html"
  | "code"
  | "table"
  | "list";

export type ParamValue = string | number | boolean | string[];

// ============================================================================
// Parameter Definition
// ============================================================================

/** 参数选项（用于select/multiselect类型） */
export interface ParamOption {
  /** 选项值 */
  value: string | number;
  /** 选项显示标签 */
  label: string;
  /** 选项描述 */
  description?: string;
}

/** 参数验证规则 */
export interface ParamValidation {
  /** 最小值（number类型） */
  min?: number;
  /** 最大值（number类型） */
  max?: number;
  /** 最小长度（string类型） */
  minLength?: number;
  /** 最大长度（string类型） */
  maxLength?: number;
  /** 正则表达式验证 */
  pattern?: string;
  /** 自定义验证错误消息 */
  message?: string;
}

/** 参数定义 */
export interface ParamDefinition {
  /** 参数名称 */
  name: string;
  /** 参数显示标签 */
  label: string;
  /** 参数类型 */
  type: ParamType;
  /** 默认值 */
  defaultValue?: string | number | boolean | string[];
  /** 参数描述 */
  description?: string;
  /** 是否必填 */
  required?: boolean;
  /** 参数占位符 */
  placeholder?: string;
  /** 选项列表（用于select/multiselect类型） */
  options?: ParamOption[];
  /** 参数验证规则 */
  validation?: ParamValidation;
  /** 参数在模板中的变量名 */
  templateVar?: string;
}

// ============================================================================
// Prompt Template
// ============================================================================

/** 提示词模板变量 */
export interface TemplateVariable {
  /** 变量名 */
  name: string;
  /** 变量描述 */
  description?: string;
  /** 变量类型 */
  type: ParamType;
  /** 默认值 */
  defaultValue?: string | number | boolean | string[];
  /** 是否必填 */
  required?: boolean;
}

/** 提示词模板定义 */
export interface PromptTemplate {
  /** 模板内容（使用{{variable}}语法） */
  template: string;
  /** 模板变量列表 */
  variables: TemplateVariable[];
  /** 模板描述 */
  description?: string;
  /** 系统提示词（可选） */
  systemPrompt?: string;
}

// ============================================================================
// Output Format
// ============================================================================

/** 输出格式定义 */
export interface OutputFormat {
  /** 输出格式类型 */
  type: OutputFormatType;
  /** 输出格式描述 */
  description?: string;
  /** 输出模式（可选，用于JSON输出） */
  schema?: Record<string, unknown>;
  /** 输出示例（可选） */
  example?: string;
}

// ============================================================================
// Command Card Definition
// ============================================================================

/** 指令卡元数据 */
export interface CommandCardMeta {
  /** 指令卡唯一标识符 */
  id: string;
  /** 指令卡名称 */
  name: string;
  /** 指令卡描述 */
  description: string;
  /** 指令卡图标（emoji或图标名称） */
  icon: string;
  /** 指令卡分类 */
  category: CommandCategory;
  /** 指令类型 */
  type: CommandType;
  /** 版本号 */
  version: string;
  /** 标签列表 */
  tags: string[];
  /** 是否为用户自定义指令卡 */
  isCustom: boolean;
  /** 创建时间 */
  createdAt?: string;
  /** 更新时间 */
  updatedAt?: string;
  /** 作者 */
  author?: string;
}

/** 指令卡定义 */
export interface CommandCardDefinition {
  /** 指令卡元数据 */
  meta: CommandCardMeta;
  /** 提示词模板 */
  prompt: PromptTemplate;
  /** 参数定义列表 */
  params: ParamDefinition[];
  /** 输出格式定义 */
  outputFormat?: OutputFormat;
  /** 快捷键（可选） */
  shortcut?: string;
  /** 指令卡优先级（用于排序） */
  priority?: number;
  /** 指令卡依赖的其他指令卡ID */
  dependencies?: string[];
  /** 扩展元数据 */
  extensions?: Record<string, unknown>;
}

// ============================================================================
// Registry Types
// ============================================================================

/** 指令卡注册表状态 */
export interface CommandCardRegistryState {
  /** 已注册的指令卡映射 */
  cards: Map<string, CommandCardDefinition>;
  /** 分类索引 */
  categoryIndex: Map<CommandCategory, Set<string>>;
  /** 标签索引 */
  tagIndex: Map<string, Set<string>>;
}

/** 指令卡查询过滤器 */
export interface CommandCardFilter {
  /** 按分类过滤 */
  category?: CommandCategory | "all";
  /** 按标签过滤 */
  tags?: string[];
  /** 按搜索关键词过滤 */
  query?: string;
  /** 是否只显示自定义指令卡 */
  customOnly?: boolean;
  /** 排序字段 */
  sortBy?: "name" | "priority" | "category" | "createdAt";
  /** 排序方向 */
  sortOrder?: "asc" | "desc";
}

/** 指令卡导入/导出格式 */
export interface CommandCardExport {
  /** 导出版本 */
  version: string;
  /** 导出时间 */
  exportedAt: string;
  /** 指令卡列表 */
  cards: CommandCardDefinition[];
}

// ============================================================================
// Legacy Compatibility Types
// ============================================================================

/**
 * 旧版CommandCardDefinition兼容接口
 * @deprecated 使用 CommandCardDefinition (新版嵌套结构) 替代
 */
export interface LegacyCommandCardDefinition {
  id: string;
  type: CommandType;
  category: CommandCategory;
  label: string;
  description: string;
  icon: string;
  params: Record<string, ParamValue>;
  promptTemplate: string;
  version: string;
  tags: string[];
  isCustom: boolean;
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * 将旧版CommandCardDefinition转换为新版格式
 */
export function migrateLegacyCard(legacy: LegacyCommandCardDefinition): CommandCardDefinition {
  const params: ParamDefinition[] = Object.entries(legacy.params).map(([key, value]) => ({
    name: key,
    label: key.charAt(0).toUpperCase() + key.slice(1).replace(/([A-Z])/g, " $1"),
    type: inferParamType(value),
    defaultValue: value,
    templateVar: key,
  }));

  const variables: TemplateVariable[] = params.map((p) => ({
    name: p.templateVar || p.name,
    type: p.type,
    defaultValue: p.defaultValue,
    description: p.description,
  }));

  return {
    meta: {
      id: legacy.id,
      name: legacy.label,
      description: legacy.description,
      icon: legacy.icon,
      category: legacy.category,
      type: legacy.type,
      version: legacy.version,
      tags: legacy.tags,
      isCustom: legacy.isCustom,
    },
    prompt: {
      template: legacy.promptTemplate,
      variables,
    },
    params,
  };
}

/**
 * 根据值推断参数类型
 */
function inferParamType(value: ParamValue): ParamType {
  if (typeof value === "number") return "number";
  if (typeof value === "boolean") return "boolean";
  if (Array.isArray(value)) return "multiselect";
  return "string";
}

/**
 * 生成指令卡唯一ID
 */
export function generateCardId(): string {
  return `custom_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
}

/**
 * 验证指令卡定义是否有效
 */
export function validateCommandCard(card: Partial<CommandCardDefinition>): string[] {
  const errors: string[] = [];

  if (!card.meta?.id) errors.push("Missing required field: meta.id");
  if (!card.meta?.name) errors.push("Missing required field: meta.name");
  if (!card.meta?.description) errors.push("Missing required field: meta.description");
  if (!card.meta?.icon) errors.push("Missing required field: meta.icon");
  if (!card.meta?.category) errors.push("Missing required field: meta.category");
  if (!card.meta?.type) errors.push("Missing required field: meta.type");
  if (!card.prompt?.template) errors.push("Missing required field: prompt.template");

  return errors;
}

// ============================================================================
// Agent Tool Types
// ============================================================================

export interface ToolInfo {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
}
