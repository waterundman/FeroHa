/**
 * Variable Resolver
 * FeroHa - Dual-Track AI Note IDE
 * Version: 2.1.8
 *
 * 支持上下文变量、用户输入变量、系统变量的解析
 */

import { ContextFragment, ContextLayer, ContextSource } from '../types/context-fragment';

// ============================================================================
// Types
// ============================================================================

/** 变量类型 */
export type VariableType = "string" | "number" | "boolean" | "array" | "object";

/** 变量定义 */
export interface VariableDefinition {
  /** 变量名 */
  name: string;
  /** 变量类型 */
  type: VariableType;
  /** 默认值 */
  defaultValue?: unknown;
  /** 变量描述 */
  description?: string;
  /** 是否必填 */
  required?: boolean;
  /** 类型转换函数 */
  converter?: (value: unknown) => unknown;
}

/** 变量上下文 */
export interface VariableContext {
  /** 用户输入变量 */
  userInputs?: Record<string, unknown>;
  /** 当前笔记上下文 */
  noteContext?: NoteContext;
  /** 系统变量覆盖 */
  systemOverrides?: Record<string, unknown>;
}

/** 笔记上下文 */
export interface NoteContext {
  /** 当前笔记内容 */
  content?: string;
  /** 选中的文本 */
  selectedText?: string;
  /** 光标位置 */
  cursorPosition?: { line: number; column: number };
  /** 笔记标题 */
  title?: string;
  /** 笔记文件名 */
  fileName?: string;
  /** 笔记路径 */
  filePath?: string;
  /** 笔记标签 */
  tags?: string[];
}

/** 解析结果 */
export interface ResolveResult {
  /** 解析后的值 */
  value: unknown;
  /** 是否使用了默认值 */
  usedDefault: boolean;
  /** 变量来源 */
  source: "user" | "note" | "system" | "default";
}

/** 解析错误 */
export interface ResolveError {
  /** 变量名 */
  variable: string;
  /** 错误消息 */
  message: string;
  /** 错误类型 */
  type: "missing" | "type_mismatch" | "conversion_error";
}

// ============================================================================
// System Variables
// ============================================================================

/** 系统变量生成器 */
const SYSTEM_VARIABLES: Record<string, () => unknown> = {
  // 日期时间
  date: () => new Date().toISOString().split("T")[0],
  time: () => new Date().toTimeString().split(" ")[0],
  datetime: () => new Date().toISOString(),
  timestamp: () => Date.now(),
  year: () => new Date().getFullYear(),
  month: () => new Date().getMonth() + 1,
  day: () => new Date().getDate(),
  weekday: () =>
    ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"][
      new Date().getDay()
    ],
  hour: () => new Date().getHours(),
  minute: () => new Date().getMinutes(),

  // 格式化日期
  dateShort: () => {
    const d = new Date();
    return `${d.getMonth() + 1}/${d.getDate()}/${d.getFullYear()}`;
  },
  dateLong: () => {
    const d = new Date();
    const months = [
      "January", "February", "March", "April", "May", "June",
      "July", "August", "September", "October", "November", "December",
    ];
    return `${months[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`;
  },
};

// ============================================================================
// VariableResolver Class
// ============================================================================

export class VariableResolver {
  private definitions: Map<string, VariableDefinition> = new Map();
  private context: VariableContext;

  constructor(context: VariableContext = {}) {
    this.context = context;
    this.registerDefaultVariables();
  }

  /**
   * 注册默认变量定义
   */
  private registerDefaultVariables(): void {
    // 系统变量
    this.register({ name: "date", type: "string", description: "当前日期 (YYYY-MM-DD)" });
    this.register({ name: "time", type: "string", description: "当前时间 (HH:MM:SS)" });
    this.register({ name: "datetime", type: "string", description: "当前日期时间" });
    this.register({ name: "timestamp", type: "number", description: "Unix时间戳" });
    this.register({ name: "year", type: "number", description: "当前年份" });
    this.register({ name: "month", type: "number", description: "当前月份" });
    this.register({ name: "day", type: "number", description: "当前日期" });
    this.register({ name: "weekday", type: "string", description: "星期几" });
    this.register({ name: "hour", type: "number", description: "当前小时" });
    this.register({ name: "minute", type: "number", description: "当前分钟" });
    this.register({ name: "dateShort", type: "string", description: "短日期格式" });
    this.register({ name: "dateLong", type: "string", description: "长日期格式" });

    // 笔记上下文变量
    this.register({ name: "note.content", type: "string", description: "当前笔记内容" });
    this.register({ name: "note.selectedText", type: "string", description: "选中的文本" });
    this.register({ name: "note.title", type: "string", description: "笔记标题" });
    this.register({ name: "note.fileName", type: "string", description: "笔记文件名" });
    this.register({ name: "note.filePath", type: "string", description: "笔记路径" });
    this.register({
      name: "note.cursorLine",
      type: "number",
      description: "光标行号",
    });
    this.register({
      name: "note.cursorColumn",
      type: "number",
      description: "光标列号",
    });
    this.register({ name: "note.tags", type: "array", description: "笔记标签" });
  }

  /**
   * 注册变量定义
   */
  register(definition: VariableDefinition): void {
    this.definitions.set(definition.name, definition);
  }

  /**
   * 批量注册变量定义
   */
  registerAll(definitions: VariableDefinition[]): void {
    definitions.forEach((def) => this.register(def));
  }

  /**
   * 更新上下文
   */
  updateContext(context: Partial<VariableContext>): void {
    this.context = { ...this.context, ...context };
  }

  /**
   * 解析单个变量
   */
  resolve(name: string, defaultValue?: unknown): ResolveResult {
    const definition = this.definitions.get(name);

    // 1. 检查用户输入
    if (this.context.userInputs && name in this.context.userInputs) {
      const value = this.context.userInputs[name];
      if (value !== undefined && value !== null && value !== "") {
        return {
          value: this.convertValue(value, definition),
          usedDefault: false,
          source: "user",
        };
      }
    }

    // 2. 检查笔记上下文
    const noteValue = this.resolveFromNoteContext(name);
    if (noteValue !== undefined) {
      return {
        value: this.convertValue(noteValue, definition),
        usedDefault: false,
        source: "note",
      };
    }

    // 3. 检查系统变量
    if (this.context.systemOverrides && name in this.context.systemOverrides) {
      const value = this.context.systemOverrides[name];
      if (value !== undefined && value !== null) {
        return {
          value: this.convertValue(value, definition),
          usedDefault: false,
          source: "system",
        };
      }
    }

    // 4. 检查系统变量生成器
    if (name in SYSTEM_VARIABLES) {
      const value = SYSTEM_VARIABLES[name]();
      return {
        value: this.convertValue(value, definition),
        usedDefault: false,
        source: "system",
      };
    }

    // 5. 使用定义中的默认值
    if (definition?.defaultValue !== undefined) {
      return {
        value: definition.defaultValue,
        usedDefault: true,
        source: "default",
      };
    }

    // 6. 使用传入的默认值
    if (defaultValue !== undefined) {
      return {
        value: defaultValue,
        usedDefault: true,
        source: "default",
      };
    }

    // 7. 变量未找到
    return {
      value: undefined,
      usedDefault: false,
      source: "default",
    };
  }

  /**
   * 从笔记上下文解析变量
   */
  private resolveFromNoteContext(name: string): unknown {
    const noteContext = this.context.noteContext;
    if (!noteContext) return undefined;

    switch (name) {
      case "note.content":
        return noteContext.content;
      case "note.selectedText":
        return noteContext.selectedText;
      case "note.title":
        return noteContext.title;
      case "note.fileName":
        return noteContext.fileName;
      case "note.filePath":
        return noteContext.filePath;
      case "note.cursorLine":
        return noteContext.cursorPosition?.line;
      case "note.cursorColumn":
        return noteContext.cursorPosition?.column;
      case "note.tags":
        return noteContext.tags;
      default:
        return undefined;
    }
  }

  /**
   * 类型转换
   */
  private convertValue(value: unknown, definition?: VariableDefinition): unknown {
    if (!definition) return value;

    // 使用自定义转换器
    if (definition.converter) {
      try {
        return definition.converter(value);
      } catch {
        return value;
      }
    }

    // 标准类型转换
    switch (definition.type) {
      case "string":
        return String(value);
      case "number": {
        const num = Number(value);
        return isNaN(num) ? value : num;
      }
      case "boolean": {
        if (typeof value === "boolean") return value;
        const str = String(value).toLowerCase();
        return str === "true" || str === "1" || str === "yes";
      }
      case "array":
        return Array.isArray(value) ? value : [value];
      case "object":
        return typeof value === "object" ? value : { value };
      default:
        return value;
    }
  }

  /**
   * 批量解析变量
   */
  resolveAll(names: string[]): Map<string, ResolveResult> {
    const results = new Map<string, ResolveResult>();
    names.forEach((name) => {
      results.set(name, this.resolve(name));
    });
    return results;
  }

  /**
   * 验证必填变量
   */
  validateRequired(names: string[]): ResolveError[] {
    const errors: ResolveError[] = [];

    names.forEach((name) => {
      const definition = this.definitions.get(name);
      if (!definition?.required) return;

      const result = this.resolve(name);
      if (result.value === undefined || result.value === null || result.value === "") {
        errors.push({
          variable: name,
          message: `Required variable '${name}' is missing`,
          type: "missing",
        });
      }
    });

    return errors;
  }

  /**
   * 获取当前上下文
   */
  getContext(): VariableContext {
    return this.context;
  }

  /**
   * 获取所有已注册的变量定义
   */
  getDefinitions(): VariableDefinition[] {
    return Array.from(this.definitions.values());
  }

  /**
   * 获取变量定义
   */
  getDefinition(name: string): VariableDefinition | undefined {
    return this.definitions.get(name);
  }

  /**
   * Convert current context into ContextFragments
   */
  toFragments(): ContextFragment[] {
    const fragments: ContextFragment[] = [];
    const now = Date.now();

    // L1 System layer — system variables and overrides
    if (this.context.systemOverrides) {
      for (const [key, value] of Object.entries(this.context.systemOverrides)) {
        if (value !== undefined && value !== null) {
          const jsonValue = JSON.stringify(value);
          fragments.push({
            id: `sys-${key}`,
            key: `system.${key}`,
            value,
            source: ContextSource.System,
            layer: ContextLayer.System,
            created_at: now,
            ttl: null,
            hash: this.simpleHash(`system.${key}:${jsonValue}`),
          });
        }
      }
    }

    // L2 Note layer — note context
    const nc = this.context.noteContext;
    if (nc) {
      const noteFields: [string, unknown][] = [
        ['note.content', nc.content],
        ['note.selectedText', nc.selectedText],
        ['note.title', nc.title],
        ['note.fileName', nc.fileName],
        ['note.filePath', nc.filePath],
        ['note.cursorLine', nc.cursorPosition?.line],
        ['note.cursorColumn', nc.cursorPosition?.column],
      ];
      for (const [key, value] of noteFields) {
        if (value !== undefined && value !== null && value !== '') {
          const jsonValue = JSON.stringify(value);
          fragments.push({
            id: `note-${key}`,
            key,
            value,
            source: ContextSource.Note,
            layer: ContextLayer.Note,
            created_at: now,
            ttl: null,
            hash: this.simpleHash(`${key}:${jsonValue}`),
          });
        }
      }
      if (nc.tags && nc.tags.length > 0) {
        const jsonValue = JSON.stringify(nc.tags);
        fragments.push({
          id: 'note-tags',
          key: 'note.tags',
          value: nc.tags,
          source: ContextSource.Note,
          layer: ContextLayer.Note,
          created_at: now,
          ttl: null,
          hash: this.simpleHash(`note.tags:${jsonValue}`),
        });
      }
    }

    // L5 Transient layer — user inputs
    if (this.context.userInputs) {
      for (const [key, value] of Object.entries(this.context.userInputs)) {
        if (value !== undefined && value !== null && value !== '') {
          const jsonValue = JSON.stringify(value);
          fragments.push({
            id: `user-${key}`,
            key: `user.${key}`,
            value,
            source: ContextSource.User,
            layer: ContextLayer.Transient,
            created_at: now,
            ttl: 5 * 60 * 1000,
            hash: this.simpleHash(`user.${key}:${jsonValue}`),
          });
        }
      }
    }

    return fragments;
  }

  private simpleHash(input: string): string {
    let hash = 0;
    for (let i = 0; i < input.length; i++) {
      const chr = input.charCodeAt(i);
      hash = ((hash << 5) - hash) + chr;
      hash |= 0;
    }
    return Math.abs(hash).toString(16).padStart(8, '0');
  }
}

// ============================================================================
// Factory Functions
// ============================================================================

/**
 * 创建默认变量解析器
 */
export function createVariableResolver(context?: VariableContext): VariableResolver {
  return new VariableResolver(context);
}

/**
 * 创建带笔记上下文的变量解析器
 */
export function createNoteVariableResolver(noteContext: NoteContext): VariableResolver {
  return new VariableResolver({ noteContext });
}
