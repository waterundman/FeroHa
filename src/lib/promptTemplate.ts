/**
 * Prompt Template Engine
 * FeroHa - Dual-Track AI Note IDE
 * Version: 2.1.8
 *
 * 支持变量插值、条件渲染、循环、函数调用
 */

import {
  VariableResolver,
  type VariableContext,
  createVariableResolver,
} from "./variableResolver";

// ============================================================================
// Types
// ============================================================================

/** 模板解析选项 */
export interface TemplateOptions {
  /** 变量解析器 */
  resolver?: VariableResolver;
  /** 变量上下文（如果没有提供resolver则创建默认的） */
  context?: VariableContext;
  /** 是否严格模式（缺少变量时抛出错误） */
  strict?: boolean;
  /** 自定义函数 */
  functions?: Record<string, TemplateFunction>;
  /** 最大递归深度（防止无限嵌套） */
  maxDepth?: number;
}

/** 模板函数类型 */
export type TemplateFunction = (...args: unknown[]) => string;

/** 模板解析结果 */
export interface TemplateResult {
  /** 解析后的内容 */
  content: string;
  /** 解析错误 */
  errors: TemplateError[];
  /** 使用的变量 */
  usedVariables: Set<string>;
}

/** 模板错误 */
export interface TemplateError {
  /** 错误消息 */
  message: string;
  /** 错误位置 */
  position?: { start: number; end: number };
  /** 错误类型 */
  type: "syntax" | "variable" | "function" | "depth";
}

// ============================================================================
// Built-in Functions
// ============================================================================

/** 内置函数 */
const BUILTIN_FUNCTIONS: Record<string, TemplateFunction> = {
  /** 格式化日期 */
  formatDate: (date?: unknown, format?: unknown) => {
    const d = date ? new Date(date as string) : new Date();
    if (isNaN(d.getTime())) return "Invalid Date";

    const fmt = (format as string) || "YYYY-MM-DD";
    const year = d.getFullYear();
    const month = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    const hours = String(d.getHours()).padStart(2, "0");
    const minutes = String(d.getMinutes()).padStart(2, "0");
    const seconds = String(d.getSeconds()).padStart(2, "0");

    return fmt
      .replace("YYYY", String(year))
      .replace("MM", month)
      .replace("DD", day)
      .replace("HH", hours)
      .replace("mm", minutes)
      .replace("ss", seconds);
  },

  /** 转大写 */
  upper: (value?: unknown) => String(value ?? "").toUpperCase(),

  /** 转小写 */
  lower: (value?: unknown) => String(value ?? "").toLowerCase(),

  /** 首字母大写 */
  capitalize: (value?: unknown) => {
    const str = String(value ?? "");
    return str.charAt(0).toUpperCase() + str.slice(1);
  },

  /** 截断文本 */
  truncate: (value?: unknown, length?: unknown) => {
    const str = String(value ?? "");
    const len = Number(length) || 100;
    if (str.length <= len) return str;
    return str.slice(0, len) + "...";
  },

  /** 去除空白 */
  trim: (value?: unknown) => String(value ?? "").trim(),

  /** 替换 */
  replace: (value?: unknown, search?: unknown, replacement?: unknown) => {
    return String(value ?? "").replace(String(search ?? ""), String(replacement ?? ""));
  },

  /** 重复 */
  repeat: (value?: unknown, count?: unknown) => {
    return String(value ?? "").repeat(Number(count) || 1);
  },

  /** 连接数组 */
  join: (value?: unknown, separator?: unknown) => {
    if (!Array.isArray(value)) return String(value ?? "");
    return value.join(String(separator ?? ", "));
  },

  /** JSON格式化 */
  json: (value?: unknown, indent?: unknown) => {
    try {
      return JSON.stringify(value, null, Number(indent) || 2);
    } catch {
      return String(value ?? "");
    }
  },

  /** 默认值 */
  default: (value?: unknown, defaultValue?: unknown) => {
    if (value === undefined || value === null || value === "") {
      return String(defaultValue ?? "");
    }
    return String(value);
  },
};

// ============================================================================
// Template Parser
// ============================================================================

/** AST节点类型 */
type NodeType =
  | "text"
  | "variable"
  | "condition"
  | "loop"
  | "function"
  | "raw";

/** AST节点 */
interface ASTNode {
  type: NodeType;
  content?: string;
  name?: string;
  args?: string[];
  children?: ASTNode[];
  elseChildren?: ASTNode[];
  itemName?: string;
  indexName?: string;
  start: number;
  end: number;
}

/**
 * 模板解析器
 */
class TemplateParser {
  private template: string;
  private pos: number;
  private errors: TemplateError[];

  constructor(template: string) {
    this.template = template;
    this.pos = 0;
    this.errors = [];
  }

  /**
   * 解析模板为AST
   */
  parse(): ASTNode[] {
    const nodes: ASTNode[] = [];

    while (this.pos < this.template.length) {
      if (this.template.startsWith("{{", this.pos)) {
        const node = this.parseTag();
        if (node) nodes.push(node);
      } else {
        // 文本节点
        const textNode = this.parseText();
        if (textNode) nodes.push(textNode);
      }
    }

    return nodes;
  }

  /**
   * 解析文本
   */
  private parseText(): ASTNode | null {
    const start = this.pos;
    const end = this.template.indexOf("{{", this.pos);

    if (end === -1) {
      this.pos = this.template.length;
      return {
        type: "text",
        content: this.template.slice(start),
        start,
        end: this.template.length,
      };
    }

    this.pos = end;
    return {
      type: "text",
      content: this.template.slice(start, end),
      start,
      end,
    };
  }

  /**
   * 解析标签
   */
  private parseTag(): ASTNode | null {
    const start = this.pos;
    this.pos += 2; // 跳过 {{

    // 跳过空白
    this.skipWhitespace();

    // 检查是否是raw标签 {{raw}}...{{/raw}}
    if (this.template.startsWith("raw}}", this.pos)) {
      return this.parseRaw(start);
    }

    // 检查是否是条件标签
    if (this.template.startsWith("#if ", this.pos)) {
      return this.parseCondition(start);
    }

    // 检查是否是循环标签
    if (this.template.startsWith("#each ", this.pos)) {
      return this.parseLoop(start);
    }

    // 检查是否是结束标签
    if (this.template.startsWith("/", this.pos)) {
      this.errors.push({
        message: "Unexpected closing tag",
        position: { start, end: this.pos + 2 },
        type: "syntax",
      });
      this.skipToTagEnd();
      return null;
    }

    // 检查是否是else标签
    if (this.template.startsWith("else}}", this.pos)) {
      this.errors.push({
        message: "Unexpected else tag",
        position: { start, end: this.pos + 6 },
        type: "syntax",
      });
      this.pos += 6;
      return null;
    }

    // 变量或函数调用
    return this.parseExpression(start);
  }

  /**
   * 解析表达式（变量或函数）
   */
  private parseExpression(start: number): ASTNode {
    const content = this.readUntil("}}");
    this.pos += 2; // 跳过 }}

    const trimmed = content.trim();

    // 检查是否是函数调用
    const funcMatch = trimmed.match(/^(\w+)\s+(.+)$/);
    if (funcMatch) {
      const funcName = funcMatch[1];
      const argsStr = funcMatch[2];
      const args = this.parseArgs(argsStr);

      return {
        type: "function",
        name: funcName,
        args,
        start,
        end: this.pos,
      };
    }

    // 变量
    return {
      type: "variable",
      name: trimmed,
      start,
      end: this.pos,
    };
  }

  /**
   * 解析raw标签
   */
  private parseRaw(start: number): ASTNode {
    this.pos += 4; // 跳过 raw}}
    const endTag = "{{/raw}}";
    const endIdx = this.template.indexOf(endTag, this.pos);

    if (endIdx === -1) {
      this.errors.push({
        message: "Unclosed raw tag",
        position: { start, end: this.template.length },
        type: "syntax",
      });
      const content = this.template.slice(this.pos);
      this.pos = this.template.length;
      return {
        type: "raw",
        content,
        start,
        end: this.pos,
      };
    }

    const content = this.template.slice(this.pos, endIdx);
    this.pos = endIdx + endTag.length;

    return {
      type: "raw",
      content,
      start,
      end: this.pos,
    };
  }

  /**
   * 解析条件标签
   */
  private parseCondition(start: number): ASTNode {
    this.pos += 4; // 跳过 #if
    this.skipWhitespace();

    const condition = this.readUntil("}}");
    this.pos += 2; // 跳过 }}

    const children: ASTNode[] = [];
    const elseChildren: ASTNode[] = [];
    let inElse = false;

    while (this.pos < this.template.length) {
      // 检查结束标签
      if (this.template.startsWith("{{/if}}", this.pos)) {
        this.pos += 7;
        return {
          type: "condition",
          name: condition.trim(),
          children,
          elseChildren,
          start,
          end: this.pos,
        };
      }

      // 检查else标签
      if (this.template.startsWith("{{else}}", this.pos)) {
        inElse = true;
        this.pos += 8;
        continue;
      }

      // 解析子节点
      const node = this.parseTag();
      if (node) {
        if (inElse) {
          elseChildren.push(node);
        } else {
          children.push(node);
        }
      } else {
        const textNode = this.parseText();
        if (textNode) {
          if (inElse) {
            elseChildren.push(textNode);
          } else {
            children.push(textNode);
          }
        }
      }
    }

    this.errors.push({
      message: "Unclosed if tag",
      position: { start, end: this.template.length },
      type: "syntax",
    });

    return {
      type: "condition",
      name: condition.trim(),
      children,
      elseChildren,
      start,
      end: this.pos,
    };
  }

  /**
   * 解析循环标签
   */
  private parseLoop(start: number): ASTNode {
    this.pos += 6; // 跳过 #each
    this.skipWhitespace();

    const expression = this.readUntil("}}");
    this.pos += 2; // 跳过 }}

    // 解析 "items as item" 或 "items as item, index"
    const parts = expression.trim().split(/\s+as\s+/);
    if (parts.length !== 2) {
      this.errors.push({
        message: "Invalid each syntax. Use: {{#each items as item}}",
        position: { start, end: this.pos },
        type: "syntax",
      });
    }

    const arrayName = parts[0]?.trim() || "";
    const itemPart = parts[1]?.trim() || "item";

    // 解析 item 和 index
    const itemParts = itemPart.split(",").map((s) => s.trim());
    const itemName = itemParts[0] || "item";
    const indexName = itemParts[1] || "index";

    const children: ASTNode[] = [];

    while (this.pos < this.template.length) {
      // 检查结束标签
      if (this.template.startsWith("{{/each}}", this.pos)) {
        this.pos += 9;
        return {
          type: "loop",
          name: arrayName,
          itemName,
          indexName,
          children,
          start,
          end: this.pos,
        };
      }

      // 解析子节点
      const node = this.parseTag();
      if (node) {
        children.push(node);
      } else {
        const textNode = this.parseText();
        if (textNode) {
          children.push(textNode);
        }
      }
    }

    this.errors.push({
      message: "Unclosed each tag",
      position: { start, end: this.template.length },
      type: "syntax",
    });

    return {
      type: "loop",
      name: arrayName,
      itemName,
      indexName,
      children,
      start,
      end: this.pos,
    };
  }

  /**
   * 解析函数参数
   */
  private parseArgs(argsStr: string): string[] {
    const args: string[] = [];
    let current = "";
    let inQuote = false;
    let quoteChar = "";

    for (let i = 0; i < argsStr.length; i++) {
      const char = argsStr[i];

      if (inQuote) {
        if (char === quoteChar) {
          inQuote = false;
        } else {
          current += char;
        }
      } else if (char === '"' || char === "'") {
        inQuote = true;
        quoteChar = char;
      } else if (char === ",") {
        args.push(current.trim());
        current = "";
      } else {
        current += char;
      }
    }

    if (current.trim()) {
      args.push(current.trim());
    }

    return args;
  }

  /**
   * 跳过空白
   */
  private skipWhitespace(): void {
    while (this.pos < this.template.length && /\s/.test(this.template[this.pos])) {
      this.pos++;
    }
  }

  /**
   * 读取直到指定字符
   */
  private readUntil(char: string): string {
    const idx = this.template.indexOf(char, this.pos);
    if (idx === -1) {
      const content = this.template.slice(this.pos);
      this.pos = this.template.length;
      return content;
    }
    const content = this.template.slice(this.pos, idx);
    this.pos = idx;
    return content;
  }

  /**
   * 跳到标签结束
   */
  private skipToTagEnd(): void {
    const idx = this.template.indexOf("}}", this.pos);
    if (idx === -1) {
      this.pos = this.template.length;
    } else {
      this.pos = idx + 2;
    }
  }

  /**
   * 获取解析错误
   */
  getErrors(): TemplateError[] {
    return this.errors;
  }
}

// ============================================================================
// Template Renderer
// ============================================================================

/**
 * 模板渲染器
 */
class TemplateRenderer {
  private resolver: VariableResolver;
  private functions: Record<string, TemplateFunction>;
  private errors: TemplateError[];
  private usedVariables: Set<string>;
  private maxDepth: number;
  private strict: boolean;

  constructor(
    resolver: VariableResolver,
    functions: Record<string, TemplateFunction>,
    maxDepth: number,
    strict: boolean
  ) {
    this.resolver = resolver;
    this.functions = { ...BUILTIN_FUNCTIONS, ...functions };
    this.errors = [];
    this.usedVariables = new Set();
    this.maxDepth = maxDepth;
    this.strict = strict;
  }

  /**
   * 渲染AST
   */
  render(nodes: ASTNode[], depth: number = 0): string {
    if (depth > this.maxDepth) {
      this.errors.push({
        message: `Maximum recursion depth (${this.maxDepth}) exceeded`,
        type: "depth",
      });
      return "";
    }

    return nodes.map((node) => this.renderNode(node, depth)).join("");
  }

  /**
   * 渲染单个节点
   */
  private renderNode(node: ASTNode, depth: number): string {
    switch (node.type) {
      case "text":
        return node.content || "";

      case "raw":
        return node.content || "";

      case "variable":
        return this.renderVariable(node);

      case "function":
        return this.renderFunction(node);

      case "condition":
        return this.renderCondition(node, depth);

      case "loop":
        return this.renderLoop(node, depth);

      default:
        return "";
    }
  }

  /**
   * 渲染变量
   */
  private renderVariable(node: ASTNode): string {
    const name = node.name || "";
    this.usedVariables.add(name);

    const result = this.resolver.resolve(name);

    if (result.value === undefined || result.value === null) {
      if (this.strict) {
        this.errors.push({
          message: `Variable '${name}' is not defined`,
          position: { start: node.start, end: node.end },
          type: "variable",
        });
      }
      return "";
    }

    return String(result.value);
  }

  /**
   * 渲染函数调用
   */
  private renderFunction(node: ASTNode): string {
    const funcName = node.name || "";
    const args = node.args || [];

    const func = this.functions[funcName];
    if (!func) {
      this.errors.push({
        message: `Function '${funcName}' is not defined`,
        position: { start: node.start, end: node.end },
        type: "function",
      });
      return "";
    }

    // 解析参数
    const resolvedArgs = args.map((arg) => {
      // 检查是否是变量引用
      if (arg.startsWith("$")) {
        const varName = arg.slice(1);
        this.usedVariables.add(varName);
        const result = this.resolver.resolve(varName);
        return result.value;
      }
      // 字面量
      return arg;
    });

    try {
      return func(...resolvedArgs);
    } catch (error) {
      this.errors.push({
        message: `Error calling function '${funcName}': ${error}`,
        position: { start: node.start, end: node.end },
        type: "function",
      });
      return "";
    }
  }

  /**
   * 渲染条件
   */
  private renderCondition(node: ASTNode, depth: number): string {
    const condition = node.name || "";
    this.usedVariables.add(condition);

    const result = this.resolver.resolve(condition);
    const isTruthy = this.isTruthy(result.value);

    if (isTruthy) {
      return this.render(node.children || [], depth + 1);
    } else {
      return this.render(node.elseChildren || [], depth + 1);
    }
  }

  /**
   * 渲染循环
   */
  private renderLoop(node: ASTNode, depth: number): string {
    const arrayName = node.name || "";
    this.usedVariables.add(arrayName);

    const result = this.resolver.resolve(arrayName);
    const array = result.value;

    if (!Array.isArray(array)) {
      if (this.strict) {
        this.errors.push({
          message: `Variable '${arrayName}' is not an array`,
          position: { start: node.start, end: node.end },
          type: "variable",
        });
      }
      return "";
    }

    const itemName = node.itemName || "item";
    const indexName = node.indexName || "index";

    // 创建子解析器，添加循环变量
    const subResolver = new VariableResolver({
      ...this.resolver.getContext(),
    });

    // 注册循环外的变量
    const definitions = this.resolver.getDefinitions();
    subResolver.registerAll(definitions);

    const results = array.map((item, index) => {
      // 注册当前项和索引
      subResolver.register({
        name: itemName,
        type: "object",
        defaultValue: item,
      });
      subResolver.register({
        name: indexName,
        type: "number",
        defaultValue: index,
      });

      // 创建子渲染器
      const subRenderer = new TemplateRenderer(
        subResolver,
        this.functions,
        this.maxDepth,
        this.strict
      );

      const rendered = subRenderer.render(node.children || [], depth + 1);

      // 收集错误和变量
      this.errors.push(...subRenderer.getErrors());
      subRenderer.getUsedVariables().forEach((v) => this.usedVariables.add(v));

      return rendered;
    });

    return results.join("");
  }

  /**
   * 判断值是否为真
   */
  private isTruthy(value: unknown): boolean {
    if (value === undefined || value === null) return false;
    if (typeof value === "boolean") return value;
    if (typeof value === "number") return value !== 0;
    if (typeof value === "string") return value.length > 0;
    if (Array.isArray(value)) return value.length > 0;
    if (typeof value === "object") return Object.keys(value).length > 0;
    return Boolean(value);
  }

  /**
   * 获取错误
   */
  getErrors(): TemplateError[] {
    return this.errors;
  }

  /**
   * 获取使用的变量
   */
  getUsedVariables(): Set<string> {
    return this.usedVariables;
  }
}

// ============================================================================
// PromptTemplateEngine Class
// ============================================================================

/**
 * 提示词模板引擎
 */
export class PromptTemplateEngine {
  private options: Required<TemplateOptions>;
  private resolver: VariableResolver;

  constructor(options: TemplateOptions = {}) {
    this.options = {
      resolver: options.resolver || createVariableResolver(options.context),
      context: options.context || {},
      strict: options.strict ?? false,
      functions: options.functions || {},
      maxDepth: options.maxDepth ?? 10,
    };
    this.resolver = this.options.resolver;
  }

  /**
   * 渲染模板
   */
  render(template: string, context?: VariableContext): TemplateResult {
    // 如果提供了新上下文，创建新的解析器
    const resolver = context
      ? createVariableResolver(context)
      : this.resolver;

    // 解析模板
    const parser = new TemplateParser(template);
    const nodes = parser.parse();
    const parseErrors = parser.getErrors();

    // 渲染模板
    const renderer = new TemplateRenderer(
      resolver,
      this.options.functions,
      this.options.maxDepth,
      this.options.strict
    );

    const content = renderer.render(nodes);

    // 合并错误
    const errors = [...parseErrors, ...renderer.getErrors()];

    return {
      content,
      errors,
      usedVariables: renderer.getUsedVariables(),
    };
  }

  /**
   * 验证模板语法
   */
  validate(template: string): TemplateError[] {
    const parser = new TemplateParser(template);
    parser.parse();
    return parser.getErrors();
  }

  /**
   * 提取模板中的变量
   */
  extractVariables(template: string): string[] {
    const variables: string[] = [];
    const regex = /\{\{([^#/][^}]*)\}\}/g;
    let match;

    while ((match = regex.exec(template)) !== null) {
      const content = match[1].trim();
      // 检查是否是函数调用
      const funcMatch = content.match(/^(\w+)\s+(.+)$/);
      if (funcMatch) {
        // 提取函数参数中的变量
        const args = funcMatch[2].split(",").map((s) => s.trim());
        args.forEach((arg) => {
          if (arg.startsWith("$")) {
            variables.push(arg.slice(1));
          }
        });
      } else {
        variables.push(content);
      }
    }

    return [...new Set(variables)];
  }

  /**
   * 更新变量解析器上下文
   */
  updateContext(context: Partial<VariableContext>): void {
    this.resolver.updateContext(context);
  }

  /**
   * 注册自定义函数
   */
  registerFunction(name: string, func: TemplateFunction): void {
    this.options.functions[name] = func;
  }

  /**
   * 批量注册自定义函数
   */
  registerFunctions(functions: Record<string, TemplateFunction>): void {
    Object.assign(this.options.functions, functions);
  }
}

// ============================================================================
// Factory Functions
// ============================================================================

/**
 * 创建默认模板引擎
 */
export function createTemplateEngine(options?: TemplateOptions): PromptTemplateEngine {
  return new PromptTemplateEngine(options);
}

/**
 * 创建带笔记上下文的模板引擎
 */
export function createNoteTemplateEngine(
  noteContext: import("./variableResolver").NoteContext
): PromptTemplateEngine {
  return new PromptTemplateEngine({
    context: { noteContext },
  });
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * 快速渲染模板
 */
export function renderTemplate(
  template: string,
  variables: Record<string, unknown>,
  options?: Omit<TemplateOptions, "context">
): string {
  const engine = createTemplateEngine(options);
  const result = engine.render(template, { userInputs: variables });
  return result.content;
}

/**
 * 检查模板语法是否有效
 */
export function isValidTemplate(template: string): boolean {
  const engine = createTemplateEngine();
  const errors = engine.validate(template);
  return errors.length === 0;
}
