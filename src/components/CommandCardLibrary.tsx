/**
 * Command Card Library
 * FeroHa - Dual-Track AI Note IDE
 * Version: 2.1.8
 */

import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import FeroHaIcon from "./FeroHaIcon";
import CommandCardLibraryItem from "./CommandCardLibraryItem";
import CommandCardPreview from "./CommandCardPreview";
import { useCommandCardStore } from "../store/commandCardStore";
import { useSettingsStore } from "../hooks/useSettings";
import type {
  CommandCardDefinition,
  CommandCategory,
  CommandType,
  ParamDefinition,
} from "../types/command-card";

// ============================================================================
// Types
// ============================================================================

interface CommandCardLibraryProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect?: (card: CommandCardDefinition) => void;
  mode?: "browse" | "select" | "manage";
}

type ViewMode = "grid" | "list";
type SortField = "name" | "category" | "createdAt" | "priority";

interface CardFormData {
  name: string;
  description: string;
  icon: string;
  category: CommandCategory;
  type: CommandType;
  tags: string[];
  template: string;
  params: ParamDefinition[];
}

export function commandCardLibraryChromeClass(mode: CommandCardLibraryProps["mode"] = "browse"): string {
  return `command-card-library ${mode === "manage" ? "embedded" : "modal"}`;
}

// ============================================================================
// Constants
// ============================================================================

const CATEGORIES: { value: CommandCategory | "all"; label: string; icon: string }[] = [
  { value: "all", label: "全部", icon: "Package" },
  { value: "content", label: "内容操作", icon: "Pencil" },
  { value: "analysis", label: "分析", icon: "Search" },
  { value: "format", label: "格式化", icon: "Ruler" },
  { value: "system", label: "系统", icon: "Settings" },
  { value: "agent", label: "Agent", icon: "Bot" },
];

const CATEGORY_COLORS: Record<CommandCategory, string> = {
  content: "#89b4fa",
  analysis: "#a6e3a1",
  format: "#f9e2af",
  system: "#f38ba8",
  agent: "#cba6f7",
};

const INITIAL_FORM_DATA: CardFormData = {
  name: "",
  description: "",
  icon: "Wrench",
  category: "content",
  type: "custom",
  tags: [],
  template: "",
  params: [],
};

// ============================================================================
// Component
// ============================================================================

export default function CommandCardLibrary({
  isOpen,
  onClose,
  onSelect,
  mode = "browse",
}: CommandCardLibraryProps) {
  // Store
  const {
    getAllCards,
    addCard,
    updateCard,
    deleteCard,
    duplicateCard,
    toggleFavorite,
    favorites,
    exportToFile,
    importFromFile,
    searchHistory,
    addSearchHistory,
    clearSearchHistory,
  } = useCommandCardStore();
  const llmReady = useSettingsStore((s) => s.settings.llmProvider === "ollama" || s.settings.llmApiKey.trim().length > 0);

  // State
  const [searchQuery, setSearchQuery] = useState("");
  const [activeCategory, setActiveCategory] = useState<CommandCategory | "all">("all");
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [sortField, setSortField] = useState<SortField>("name");
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">("asc");
  const [showCustomOnly, setShowCustomOnly] = useState(false);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [selectedPreviewCard, setSelectedPreviewCard] = useState<CommandCardDefinition | null>(null);
  const [previewMessage, setPreviewMessage] = useState("");

  // Editor state
  const [isEditorOpen, setIsEditorOpen] = useState(false);
  const [editingCard, setEditingCard] = useState<CommandCardDefinition | null>(null);
  const [formData, setFormData] = useState<CardFormData>(INITIAL_FORM_DATA);
  const [formErrors, setFormErrors] = useState<string[]>([]);

  // Refs
  const searchInputRef = useRef<HTMLInputElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // ============================================================================
  // Effects
  // ============================================================================

  useEffect(() => {
    if (isOpen && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (isEditorOpen) {
          setIsEditorOpen(false);
        } else {
          onClose();
        }
      }
      if (e.key === "/" && !isEditorOpen) {
        e.preventDefault();
        searchInputRef.current?.focus();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose, isEditorOpen]);

  // ============================================================================
  // Computed
  // ============================================================================

  const allCards = useMemo(() => {
    return getAllCards({
      category: activeCategory !== "all" ? activeCategory : undefined,
      tags: selectedTags.length > 0 ? selectedTags : undefined,
      query: searchQuery,
      customOnly: showCustomOnly,
      sortBy: sortField,
      sortOrder,
    });
  }, [getAllCards, activeCategory, selectedTags, searchQuery, showCustomOnly, sortField, sortOrder]);

  const allTags = useMemo(() => {
    const tags = new Set<string>();
    allCards.forEach((card) => card.meta.tags.forEach((tag) => tags.add(tag)));
    return Array.from(tags).sort();
  }, [allCards]);

  const groupedCards = useMemo(() => {
    const groups: Record<string, CommandCardDefinition[]> = {};
    allCards.forEach((card) => {
      const category = card.meta.category;
      if (!groups[category]) {
        groups[category] = [];
      }
      groups[category].push(card);
    });
    return groups;
  }, [allCards]);

  // ============================================================================
  // Handlers
  // ============================================================================

  const handleSearch = useCallback(
    (query: string) => {
      setSearchQuery(query);
      if (query.trim()) {
        addSearchHistory(query.trim());
      }
    },
    [addSearchHistory]
  );

  const handleCategoryChange = useCallback((category: CommandCategory | "all") => {
    setActiveCategory(category);
  }, []);

  const handleTagToggle = useCallback((tag: string) => {
    setSelectedTags((prev) =>
      prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag]
    );
  }, []);

  const handleCardClick = useCallback(
    (card: CommandCardDefinition) => {
      if (mode === "select" && onSelect) {
        onSelect(card);
        onClose();
        return;
      }
      setSelectedPreviewCard(card);
      setPreviewMessage("");
    },
    [mode, onSelect, onClose]
  );

  const handleUsePreviewCard = useCallback(() => {
    if (!selectedPreviewCard) return;
    if (onSelect) {
      onSelect(selectedPreviewCard);
      onClose();
      return;
    }
    void navigator.clipboard?.writeText(selectedPreviewCard.prompt.template);
    setPreviewMessage("Prompt template copied");
  }, [selectedPreviewCard, onSelect, onClose]);

  const handleCreateCard = useCallback(() => {
    setEditingCard(null);
    setFormData(INITIAL_FORM_DATA);
    setFormErrors([]);
    setIsEditorOpen(true);
  }, []);

  const handleEditCard = useCallback((card: CommandCardDefinition) => {
    setEditingCard(card);
    setFormData({
      name: card.meta.name,
      description: card.meta.description,
      icon: card.meta.icon,
      category: card.meta.category,
      type: card.meta.type,
      tags: card.meta.tags,
      template: card.prompt.template,
      params: card.params,
    });
    setFormErrors([]);
    setIsEditorOpen(true);
  }, []);

  const handleDeleteCard = useCallback(
    (id: string) => {
      if (window.confirm("确定要删除这个指令卡吗？")) {
        deleteCard(id);
      }
    },
    [deleteCard]
  );

  const handleDuplicateCard = useCallback(
    (id: string) => {
      duplicateCard(id);
    },
    [duplicateCard]
  );

  const handleToggleFavorite = useCallback(
    (id: string) => {
      toggleFavorite(id);
    },
    [toggleFavorite]
  );

  const handleFormSubmit = useCallback(() => {
    const errors: string[] = [];

    if (!formData.name.trim()) {
      errors.push("名称不能为空");
    }
    if (!formData.description.trim()) {
      errors.push("描述不能为空");
    }
    if (!formData.template.trim()) {
      errors.push("提示词模板不能为空");
    }

    if (errors.length > 0) {
      setFormErrors(errors);
      return;
    }

    if (editingCard) {
      // 更新现有卡
      const success = updateCard(editingCard.meta.id, {
        meta: {
          ...editingCard.meta,
          name: formData.name,
          description: formData.description,
          icon: formData.icon,
          category: formData.category,
          type: formData.type,
          tags: formData.tags,
        },
        prompt: {
          template: formData.template,
          variables: formData.params.map((p) => ({
            name: p.templateVar || p.name,
            type: p.type,
            defaultValue: p.defaultValue,
            description: p.description,
            required: p.required,
          })),
        },
        params: formData.params,
      });

      if (success) {
        setIsEditorOpen(false);
      } else {
        setFormErrors(["更新失败"]);
      }
    } else {
      // 创建新卡
      const success = addCard({
        name: formData.name,
        description: formData.description,
        icon: formData.icon,
        category: formData.category,
        type: formData.type,
        tags: formData.tags,
        version: "1.0.0",
        isCustom: true,
        prompt: {
          template: formData.template,
          variables: formData.params.map((p) => ({
            name: p.templateVar || p.name,
            type: p.type,
            defaultValue: p.defaultValue,
            description: p.description,
            required: p.required,
          })),
        },
        params: formData.params,
      });

      if (success) {
        setIsEditorOpen(false);
      } else {
        setFormErrors(["创建失败"]);
      }
    }
  }, [formData, editingCard, addCard, updateCard]);

  const handleExport = useCallback(() => {
    exportToFile(showCustomOnly);
  }, [exportToFile, showCustomOnly]);

  const handleImport = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;

      const result = await importFromFile(file);
      if (result.success > 0) {
        alert(`成功导入 ${result.success} 个指令卡`);
      }
      if (result.failed > 0) {
        alert(`导入失败 ${result.failed} 个: ${result.errors.join(", ")}`);
      }

      // 重置文件输入
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    },
    [importFromFile]
  );

  const handleAddParam = useCallback(() => {
    const newParam: ParamDefinition = {
      name: `param${formData.params.length + 1}`,
      label: `参数${formData.params.length + 1}`,
      type: "string",
      required: false,
      templateVar: `param${formData.params.length + 1}`,
    };
    setFormData((prev) => ({
      ...prev,
      params: [...prev.params, newParam],
    }));
  }, [formData.params.length]);

  const handleRemoveParam = useCallback((index: number) => {
    setFormData((prev) => ({
      ...prev,
      params: prev.params.filter((_, i) => i !== index),
    }));
  }, []);

  const handleParamChange = useCallback(
    (
      index: number,
      field: keyof ParamDefinition,
      value: ParamDefinition[keyof ParamDefinition]
    ) => {
      setFormData((prev) => ({
        ...prev,
        params: prev.params.map((param, i) =>
          i === index ? { ...param, [field]: value } : param
        ),
      }));
    },
    []
  );

  // ============================================================================
  // Render Helpers
  // ============================================================================

  const renderSearchBar = () => (
    <div className="search-section">
      <div className="search-bar">
        <span className="search-icon"><FeroHaIcon name="Search" size={16} /></span>
        <input
          ref={searchInputRef}
          type="text"
          placeholder="搜索指令卡... (按 / 聚焦)"
          value={searchQuery}
          onChange={(e) => handleSearch(e.target.value)}
          className="search-input"
        />
        {searchQuery && (
          <button
            className="clear-search"
            onClick={() => setSearchQuery("")}
            aria-label="清除搜索"
          >
            <FeroHaIcon name="X" size={14} />
          </button>
        )}
      </div>

      {searchHistory.length > 0 && !searchQuery && (
        <div className="search-history">
          <div className="history-header">
            <span className="history-label">搜索历史</span>
            <button className="clear-history" onClick={clearSearchHistory}>
              清除
            </button>
          </div>
          <div className="history-tags">
            {searchHistory.slice(0, 8).map((query) => (
              <button
                key={query}
                className="history-tag"
                onClick={() => handleSearch(query)}
              >
                {query}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );

  const renderFilters = () => (
    <div className="filters-section">
      <div className="category-tabs">
        {CATEGORIES.map(({ value, label, icon }) => (
          <button
            key={value}
            className={`category-tab ${activeCategory === value ? "active" : ""}`}
            onClick={() => handleCategoryChange(value)}
          >
            <span className="tab-icon"><FeroHaIcon name={icon} size={14} /></span>
            <span className="tab-label">{label}</span>
          </button>
        ))}
      </div>

      <div className="filter-controls">
        <div className="view-toggle">
          <button
            className={`view-btn ${viewMode === "grid" ? "active" : ""}`}
            onClick={() => setViewMode("grid")}
            title="网格视图"
          >
            <FeroHaIcon name="LayoutGrid" size={16} />
          </button>
          <button
            className={`view-btn ${viewMode === "list" ? "active" : ""}`}
            onClick={() => setViewMode("list")}
            title="列表视图"
          >
            <FeroHaIcon name="AlignJustify" size={16} />
          </button>
        </div>

        <select
          className="sort-select"
          value={`${sortField}-${sortOrder}`}
          onChange={(e) => {
            const [field, order] = e.target.value.split("-") as [SortField, "asc" | "desc"];
            setSortField(field);
            setSortOrder(order);
          }}
        >
          <option value="name-asc">名称 A-Z</option>
          <option value="name-desc">名称 Z-A</option>
          <option value="category-asc">分类</option>
          <option value="createdAt-desc">最新创建</option>
          <option value="priority-desc">优先级</option>
        </select>

        <label className="custom-only-toggle">
          <input
            type="checkbox"
            checked={showCustomOnly}
            onChange={(e) => setShowCustomOnly(e.target.checked)}
          />
          <span>仅自定义</span>
        </label>
      </div>
    </div>
  );

  const renderTags = () => {
    if (allTags.length === 0) return null;

    return (
      <div className="tags-section">
        <div className="tags-scroll">
          {allTags.map((tag) => (
            <button
              key={tag}
              className={`tag-btn ${selectedTags.includes(tag) ? "active" : ""}`}
              onClick={() => handleTagToggle(tag)}
            >
              {tag}
            </button>
          ))}
        </div>
      </div>
    );
  };

  const renderCardGrid = () => (
    <div className={`cards-container ${viewMode}`}>
      {Object.entries(groupedCards).map(([category, cards]) => (
        <div key={category} className="category-section">
          <h4 className="category-title">
            <span
              className="category-indicator"
              style={{ backgroundColor: CATEGORY_COLORS[category as CommandCategory] }}
            />
            <FeroHaIcon name={CATEGORIES.find((c) => c.value === category)?.icon || "Package"} size={14} />{" "}
            {CATEGORIES.find((c) => c.value === category)?.label || category}
            <span className="category-count">{cards.length}</span>
          </h4>

          <div className={`cards-grid ${viewMode}`}>
            {cards.map((card) => (
              <CommandCardLibraryItem
                key={card.meta.id}
                card={card}
                selected={selectedPreviewCard?.meta.id === card.meta.id}
                favorite={favorites.includes(card.meta.id)}
                llmReady={llmReady}
                onOpen={handleCardClick}
                onToggleFavorite={handleToggleFavorite}
                onEdit={handleEditCard}
                onDelete={handleDeleteCard}
                onDuplicate={handleDuplicateCard}
              />
            ))}
          </div>
        </div>
      ))}

      {allCards.length === 0 && (
        <div className="no-results">
          <span className="no-results-icon"><FeroHaIcon name="Search" size={48} /></span>
          <p className="no-results-text">未找到匹配的指令卡</p>
          <p className="no-results-hint">尝试调整搜索条件或创建新的指令卡</p>
        </div>
      )}
    </div>
  );

  const renderEditor = () => {
    if (!isEditorOpen) return null;

    return (
      <div className="editor-overlay">
        <div className="editor-panel">
          <div className="editor-header">
            <h3>{editingCard ? "编辑指令卡" : "创建指令卡"}</h3>
            <button className="close-btn" onClick={() => setIsEditorOpen(false)}>
              <FeroHaIcon name="X" size={14} />
            </button>
          </div>

          <div className="editor-content">
            {formErrors.length > 0 && (
              <div className="form-errors">
                {formErrors.map((error) => (
                  <p key={error} className="error-message">
                    <FeroHaIcon name="AlertTriangle" size={14} /> {error}
                  </p>
                ))}
              </div>
            )}

            <div className="form-group">
              <label className="form-label">名称 *</label>
              <input
                type="text"
                className="form-input"
                value={formData.name}
                onChange={(e) => setFormData((prev) => ({ ...prev, name: e.target.value }))}
                placeholder="输入指令卡名称"
              />
            </div>

            <div className="form-group">
              <label className="form-label">描述 *</label>
              <textarea
                className="form-textarea"
                value={formData.description}
                onChange={(e) =>
                  setFormData((prev) => ({ ...prev, description: e.target.value }))
                }
                placeholder="输入指令卡描述"
                rows={3}
              />
            </div>

            <div className="form-row">
              <div className="form-group">
                <label className="form-label">图标</label>
                <input
                  type="text"
                  className="form-input icon-input"
                  value={formData.icon}
                  onChange={(e) => setFormData((prev) => ({ ...prev, icon: e.target.value }))}
                  placeholder="Wrench"
                />
              </div>

              <div className="form-group">
                <label className="form-label">分类</label>
                <select
                  className="form-select"
                  value={formData.category}
                  onChange={(e) =>
                    setFormData((prev) => ({
                      ...prev,
                      category: e.target.value as CommandCategory,
                    }))
                  }
                >
                  {CATEGORIES.filter((c) => c.value !== "all").map(({ value, label }) => (
                    <option key={value} value={value}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div className="form-group">
              <label className="form-label">标签</label>
              <div className="tags-input">
                {formData.tags.map((tag) => (
                  <span key={tag} className="tag-item">
                    {tag}
                    <button
                      className="tag-remove"
                      onClick={() =>
                        setFormData((prev) => ({
                          ...prev,
                          tags: prev.tags.filter((t) => t !== tag),
                        }))
                      }
                    >
                          <FeroHaIcon name="X" size={12} />
                    </button>
                  </span>
                ))}
                <input
                  type="text"
                  className="tag-input"
                  placeholder="输入标签后按回车"
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && e.currentTarget.value.trim()) {
                      e.preventDefault();
                      const newTag = e.currentTarget.value.trim();
                      if (!formData.tags.includes(newTag)) {
                        setFormData((prev) => ({
                          ...prev,
                          tags: [...prev.tags, newTag],
                        }));
                      }
                      e.currentTarget.value = "";
                    }
                  }}
                />
              </div>
            </div>

            <div className="form-group">
              <label className="form-label">提示词模板 *</label>
              <textarea
                className="form-textarea template-textarea"
                value={formData.template}
                onChange={(e) =>
                  setFormData((prev) => ({ ...prev, template: e.target.value }))
                }
                placeholder="使用 {{变量名}} 语法定义变量，例如: 总结以下内容: {{content}}"
                rows={4}
              />
              <p className="form-hint">使用 {`{{变量名}}`} 语法定义变量</p>
            </div>

            <div className="form-group">
              <div className="params-header">
                <label className="form-label">参数定义</label>
                <button className="add-param-btn" onClick={handleAddParam}>
                  + 添加参数
                </button>
              </div>

              {formData.params.length === 0 ? (
                <p className="no-params">暂无参数定义</p>
              ) : (
                <div className="params-list">
                  {formData.params.map((param, index) => (
                    <div key={index} className="param-item">
                      <div className="param-fields">
                        <input
                          type="text"
                          className="form-input param-name"
                          value={param.name}
                          onChange={(e) => handleParamChange(index, "name", e.target.value)}
                          placeholder="参数名"
                        />
                        <input
                          type="text"
                          className="form-input param-label"
                          value={param.label}
                          onChange={(e) => handleParamChange(index, "label", e.target.value)}
                          placeholder="显示标签"
                        />
                        <select
                          className="form-select param-type"
                          value={param.type}
                          onChange={(e) => handleParamChange(index, "type", e.target.value)}
                        >
                          <option value="string">文本</option>
                          <option value="number">数字</option>
                          <option value="boolean">布尔</option>
                          <option value="select">下拉选择</option>
                          <option value="textarea">多行文本</option>
                        </select>
                        <label className="param-required">
                          <input
                            type="checkbox"
                            checked={param.required || false}
                            onChange={(e) =>
                              handleParamChange(index, "required", e.target.checked)
                            }
                          />
                          <span>必填</span>
                        </label>
                        <button
                          className="remove-param-btn"
                          onClick={() => handleRemoveParam(index)}
                        >
                      <FeroHaIcon name="X" size={10} />
                        </button>
                      </div>
                      <input
                        type="text"
                        className="form-input param-placeholder"
                        value={param.placeholder || ""}
                        onChange={(e) =>
                          handleParamChange(index, "placeholder", e.target.value)
                        }
                        placeholder="占位符文本"
                      />
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className="editor-footer">
            <button className="cancel-btn" onClick={() => setIsEditorOpen(false)}>
              取消
            </button>
            <button className="save-btn" onClick={handleFormSubmit}>
              {editingCard ? "保存修改" : "创建指令卡"}
            </button>
          </div>
        </div>
      </div>
    );
  };

  // ============================================================================
  // Main Render
  // ============================================================================

  if (!isOpen) return null;

  return (
    <div className={commandCardLibraryChromeClass(mode)}>
      <div className="library-overlay" onClick={onClose} />
      <div className="library-panel">
        <div className="library-header">
          <div className="header-left">
            <h2 className="library-title">指令卡库</h2>
            <span className="card-count">{allCards.length} 个指令卡</span>
          </div>

          <div className="header-actions">
            {mode === "manage" && (
              <>
                <button className="action-btn create-btn" onClick={handleCreateCard}>
                  + 创建指令卡
                </button>
                <button className="action-btn export-btn" onClick={handleExport}>
                  <><FeroHaIcon name="Download" size={14} /> 导出</>
                </button>
                <button
                  className="action-btn import-btn"
                  onClick={() => fileInputRef.current?.click()}
                >
                  <><FeroHaIcon name="Upload" size={14} /> 导入</>
                </button>
                <input
                  ref={fileInputRef}
                  type="file"
                  accept=".json"
                  onChange={handleImport}
                  style={{ display: "none" }}
                />
              </>
            )}
            <button className="close-btn" onClick={onClose} aria-label="关闭">
              <FeroHaIcon name="X" size={14} />
            </button>
          </div>
        </div>

        <div className="library-content">
          {renderSearchBar()}
          {renderFilters()}
          {renderTags()}
          <div className="library-main">
            {renderCardGrid()}
            <CommandCardPreview
              card={selectedPreviewCard}
              llmReady={llmReady}
              useLabel={onSelect ? "Use card" : "Copy template"}
              message={previewMessage}
              onUse={handleUsePreviewCard}
            />
          </div>
        </div>

        {renderEditor()}
      </div>

      <style>{`
        .command-card-library {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          z-index: 1000;
          display: flex;
          align-items: center;
          justify-content: center;
        }

        .command-card-library.embedded {
          position: relative;
          inset: auto;
          z-index: auto;
          width: 100%;
          height: 100%;
          align-items: stretch;
          justify-content: stretch;
          background: var(--bg-primary);
        }

        .library-overlay {
          position: absolute;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: rgba(0, 0, 0, 0.6);
          backdrop-filter: blur(4px);
        }

        .command-card-library.embedded .library-overlay {
          display: none;
        }

        .library-panel {
          position: relative;
          background: var(--bg-secondary);
          border-radius: 8px;
          width: 90%;
          max-width: 1200px;
          height: 85vh;
          display: flex;
          flex-direction: column;
          box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
          border: 1px solid var(--border-color);
          animation: librarySlideIn 0.3s ease;
        }

        .command-card-library.embedded .library-panel {
          width: 100%;
          max-width: none;
          height: 100%;
          border-radius: 0;
          border: 0;
          box-shadow: none;
          animation: none;
        }

        @keyframes librarySlideIn {
          from {
            opacity: 0;
            transform: translateY(20px) scale(0.95);
          }
          to {
            opacity: 1;
            transform: translateY(0) scale(1);
          }
        }

        .library-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 16px 24px;
          border-bottom: 1px solid var(--border-color);
          background: var(--bg-primary);
        }

        .header-left {
          display: flex;
          align-items: center;
          gap: 12px;
        }

        .library-title {
          margin: 0;
          font-size: 20px;
          font-weight: 600;
          color: var(--text-primary);
        }

        .card-count {
          font-size: 14px;
          color: var(--text-muted);
          padding: 4px 8px;
          background: var(--bg-input);
          border-radius: 6px;
        }

        .header-actions {
          display: flex;
          gap: 8px;
          align-items: center;
        }

        .action-btn {
          padding: 8px 12px;
          border: none;
          border-radius: 6px;
          cursor: pointer;
          font-size: 13px;
          font-weight: 500;
          transition: all 0.15s;
          display: flex;
          align-items: center;
          gap: 6px;
        }

        .create-btn {
          background: var(--accent-primary);
          color: var(--bg-primary);
        }

        .create-btn:hover {
          background: var(--accent-primary);
          box-shadow: 0 0 12px var(--accent-glow);
        }

        .export-btn,
        .import-btn {
          background: var(--bg-input);
          color: var(--text-secondary);
          border: 1px solid var(--border-color);
        }

        .export-btn:hover,
        .import-btn:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .close-btn {
          background: transparent;
          border: none;
          color: var(--text-muted);
          font-size: 18px;
          cursor: pointer;
          padding: 8px;
          border-radius: 6px;
          transition: all 0.15s;
        }

        .close-btn:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .library-content {
          flex: 1;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        .library-main {
          flex: 1;
          min-height: 0;
          display: grid;
          grid-template-columns: minmax(0, 1fr) minmax(260px, 320px);
          overflow: hidden;
        }

        .command-card-preview {
          border-left: 1px solid var(--border-color);
          background: var(--bg-primary);
          padding: 16px;
          overflow: auto;
          color: var(--text-secondary);
          display: flex;
          flex-direction: column;
          gap: 14px;
        }

        .command-card-preview.empty {
          align-items: center;
          justify-content: center;
          text-align: center;
          color: var(--text-muted);
          padding: 24px;
        }

        .preview-header {
          display: flex;
          gap: 10px;
          align-items: flex-start;
        }

        .preview-icon {
          color: var(--accent-primary);
          flex: 0 0 auto;
        }

        .preview-header h3 {
          margin: 0 0 4px 0;
          color: var(--text-primary);
          font-size: 16px;
        }

        .preview-header p,
        .command-card-preview.empty p {
          margin: 0;
          font-size: 12px;
          line-height: 1.5;
        }

        .preview-skill {
          display: flex;
          align-items: center;
          gap: 7px;
          font-size: 11px;
          color: var(--text-muted);
          padding: 8px;
          border: 1px solid var(--border-color);
          border-radius: 6px;
          background: var(--bg-input);
        }

        .preview-skill strong {
          color: var(--accent-primary);
          font-weight: 600;
        }

        .preview-block {
          display: flex;
          flex-direction: column;
          gap: 6px;
        }

        .preview-label {
          color: var(--text-muted);
          font-size: 11px;
          font-weight: 700;
          text-transform: uppercase;
          letter-spacing: 0.04em;
        }

        .preview-block pre {
          margin: 0;
          white-space: pre-wrap;
          word-break: break-word;
          color: var(--text-primary);
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 6px;
          padding: 10px;
          font-size: 12px;
          line-height: 1.45;
          max-height: 220px;
          overflow: auto;
        }

        .preview-block ul {
          list-style: none;
          padding: 0;
          margin: 0;
          display: flex;
          flex-direction: column;
          gap: 6px;
        }

        .preview-block li {
          display: flex;
          justify-content: space-between;
          gap: 8px;
          padding: 7px 8px;
          border: 1px solid var(--border-color);
          border-radius: 6px;
          background: var(--bg-input);
          font-size: 12px;
        }

        .preview-block li strong {
          color: var(--text-primary);
          font-weight: 600;
        }

        .preview-muted {
          margin: 0;
          color: var(--text-muted);
          font-size: 12px;
        }

        .preview-actions {
          display: flex;
          align-items: center;
          gap: 10px;
          margin-top: auto;
        }

        .use-preview-btn {
          border: 0;
          border-radius: 6px;
          background: var(--accent-primary);
          color: var(--bg-primary);
          padding: 8px 12px;
          font-size: 12px;
          font-weight: 700;
          cursor: pointer;
        }

        .preview-message {
          color: var(--accent-primary);
          font-size: 12px;
        }

        .search-section {
          padding: 16px 24px;
          border-bottom: 1px solid var(--border-color);
        }

        .search-bar {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 12px 16px;
          background: var(--bg-input);
          border-radius: 8px;
          border: 1px solid var(--border-color);
          transition: all 0.15s;
        }

        .search-bar:focus-within {
          border-color: var(--accent-primary);
          box-shadow: 0 0 0 2px var(--accent-glow);
        }

        .search-icon {
          color: var(--text-muted);
          font-size: 16px;
        }

        .search-input {
          flex: 1;
          background: transparent;
          border: none;
          outline: none;
          color: var(--text-primary);
          font-size: 14px;
          font-family: inherit;
        }

        .search-input::placeholder {
          color: var(--text-muted);
        }

        .clear-search {
          background: transparent;
          border: none;
          color: var(--text-muted);
          cursor: pointer;
          padding: 4px;
          border-radius: 4px;
          font-size: 12px;
        }

        .clear-search:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .search-history {
          margin-top: 12px;
        }

        .history-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 8px;
        }

        .history-label {
          font-size: 12px;
          color: var(--text-muted);
        }

        .clear-history {
          background: transparent;
          border: none;
          color: var(--text-muted);
          cursor: pointer;
          font-size: 12px;
          padding: 2px 6px;
          border-radius: 4px;
        }

        .clear-history:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .history-tags {
          display: flex;
          flex-wrap: wrap;
          gap: 6px;
        }

        .history-tag {
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          color: var(--text-secondary);
          padding: 4px 8px;
          border-radius: 4px;
          cursor: pointer;
          font-size: 12px;
          transition: all 0.15s;
        }

        .history-tag:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .filters-section {
          padding: 12px 24px;
          border-bottom: 1px solid var(--border-color);
          display: flex;
          justify-content: space-between;
          align-items: center;
          gap: 16px;
          flex-wrap: wrap;
        }

        .category-tabs {
          display: flex;
          gap: 4px;
          overflow-x: auto;
        }

        .category-tab {
          background: transparent;
          border: none;
          color: var(--text-muted);
          padding: 8px 12px;
          border-radius: 6px;
          cursor: pointer;
          font-size: 13px;
          white-space: nowrap;
          transition: all 0.15s;
          display: flex;
          align-items: center;
          gap: 6px;
        }

        .category-tab:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .category-tab.active {
          background: var(--bg-input);
          color: var(--accent-primary);
        }

        .tab-icon {
          font-size: 14px;
        }

        .filter-controls {
          display: flex;
          align-items: center;
          gap: 12px;
          flex-wrap: wrap;
        }

        .view-toggle {
          display: flex;
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 6px;
          overflow: hidden;
        }

        .view-btn {
          background: transparent;
          border: none;
          color: var(--text-muted);
          padding: 8px 12px;
          cursor: pointer;
          font-size: 14px;
          transition: all 0.15s;
        }

        .view-btn:hover {
          color: var(--text-primary);
        }

        .view-btn.active {
          background: var(--accent-secondary);
          color: var(--accent-primary);
        }

        .sort-select {
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          color: var(--text-primary);
          padding: 8px 12px;
          border-radius: 6px;
          font-size: 13px;
          cursor: pointer;
        }

        .sort-select:focus {
          outline: none;
          border-color: var(--accent-primary);
        }

        .custom-only-toggle {
          display: flex;
          align-items: center;
          gap: 6px;
          color: var(--text-secondary);
          font-size: 13px;
          cursor: pointer;
        }

        .custom-only-toggle input {
          accent-color: var(--accent-primary);
        }

        .tags-section {
          padding: 12px 24px;
          border-bottom: 1px solid var(--border-color);
        }

        .tags-scroll {
          display: flex;
          flex-wrap: wrap;
          gap: 6px;
        }

        .tag-btn {
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          color: var(--text-secondary);
          padding: 6px 10px;
          border-radius: 4px;
          cursor: pointer;
          font-size: 12px;
          transition: all 0.15s;
        }

        .tag-btn:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .tag-btn.active {
          background: var(--accent-primary);
          color: var(--bg-primary);
        }

        .cards-container {
          flex: 1;
          overflow-y: auto;
          padding: 16px 24px;
        }

        .category-section {
          margin-bottom: 24px;
        }

        .category-title {
          margin: 0 0 12px 0;
          font-size: 14px;
          font-weight: 600;
          color: var(--text-secondary);
          display: flex;
          align-items: center;
          gap: 8px;
        }

        .category-indicator {
          width: 4px;
          height: 16px;
          border-radius: 2px;
        }

        .category-count {
          font-size: 12px;
          color: var(--text-muted);
          font-weight: normal;
        }

        .cards-grid {
          display: grid;
          gap: 12px;
        }

        .cards-grid.grid {
          grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
        }

        .cards-grid.list {
          grid-template-columns: 1fr;
        }

        .card-item {
          background: var(--bg-input);
          border-radius: 8px;
          padding: 16px;
          cursor: pointer;
          transition: all 0.2s ease;
          border: 1px solid var(--border-color);
          position: relative;
        }

        .card-item:hover {
          background: var(--bg-hover);
          border-color: var(--accent-primary);
          transform: translateY(-1px);
          box-shadow: 0 8px 18px rgba(0, 0, 0, 0.25);
        }

        .card-item.selected {
          border-color: var(--accent-primary);
          box-shadow: 0 0 0 1px var(--accent-primary), 0 8px 18px rgba(0, 0, 0, 0.25);
        }

        .card-item.favorite {
          border-color: var(--diff-warn);
        }

        .card-item.custom {
          border-left: 3px solid var(--accent-primary);
        }

        .card-header {
          display: flex;
          justify-content: space-between;
          align-items: flex-start;
          margin-bottom: 12px;
        }

        .card-icon {
          font-size: 28px;
          line-height: 1;
        }

        .card-actions {
          display: flex;
          gap: 4px;
          opacity: 0;
          transition: opacity 0.15s;
        }

        .card-item:hover .card-actions {
          opacity: 1;
        }

        .action-btn {
          background: transparent;
          border: none;
          color: var(--text-muted);
          cursor: pointer;
          padding: 4px 6px;
          border-radius: 4px;
          font-size: 12px;
          transition: all 0.15s;
        }

        .action-btn:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .favorite-btn.active {
          color: var(--diff-warn);
        }

        .edit-btn:hover {
          color: var(--accent-primary);
        }

        .delete-btn:hover {
          color: var(--diff-delete);
        }

        .card-body {
          margin-bottom: 12px;
        }

        .card-title {
          margin: 0 0 6px 0;
          font-size: 15px;
          font-weight: 600;
          color: var(--text-primary);
        }

        .card-description {
          margin: 0;
          font-size: 12px;
          color: var(--text-secondary);
          line-height: 1.5;
          display: -webkit-box;
          -webkit-line-clamp: 2;
          -webkit-box-orient: vertical;
          overflow: hidden;
        }

        .card-skill-line {
          display: flex;
          align-items: center;
          gap: 6px;
          margin-top: 8px;
          color: var(--text-muted);
          font-size: 10px;
          line-height: 1.3;
          min-width: 0;
        }

        .card-skill-line span:nth-child(2) {
          min-width: 0;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .card-skill-line span:nth-child(3) {
          flex: 0 0 auto;
          color: var(--accent-primary);
        }

        .card-skill-dot {
          width: 6px;
          height: 6px;
          border-radius: 50%;
          background: var(--accent-primary);
          flex: 0 0 auto;
        }

        .card-skill-dot.needs-api {
          background: var(--diff-warn);
        }

        .card-footer {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 8px;
        }

        .card-tags {
          display: flex;
          flex-wrap: wrap;
          gap: 4px;
        }

        .card-tag {
          font-size: 10px;
          padding: 2px 6px;
          background: var(--bg-secondary);
          border-radius: 4px;
          color: var(--text-secondary);
        }

        .card-tag-more {
          font-size: 10px;
          padding: 2px 6px;
          color: var(--text-muted);
        }

        .custom-badge {
          font-size: 10px;
          padding: 2px 6px;
          background: var(--accent-primary);
          border-radius: 4px;
          color: var(--bg-primary);
          font-weight: 600;
        }

        .card-meta {
          display: flex;
          justify-content: space-between;
          align-items: center;
          font-size: 11px;
          color: var(--text-muted);
        }

        .meta-version {
          font-family: 'JetBrains Mono', monospace;
        }

        .no-results {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding: 60px 20px;
          color: var(--text-muted);
        }

        .no-results-icon {
          font-size: 48px;
          margin-bottom: 16px;
        }

        .no-results-text {
          margin: 0 0 8px 0;
          font-size: 16px;
          color: var(--text-secondary);
        }

        .no-results-hint {
          margin: 0;
          font-size: 14px;
        }

        /* Editor Styles */
        .editor-overlay {
          position: absolute;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: rgba(0, 0, 0, 0.7);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 10;
        }

        .editor-panel {
          background: var(--bg-secondary);
          border-radius: 8px;
          width: 90%;
          max-width: 600px;
          max-height: 90%;
          display: flex;
          flex-direction: column;
          box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
          border: 1px solid var(--border-color);
        }

        .editor-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 16px 24px;
          border-bottom: 1px solid var(--border-color);
        }

        .editor-header h3 {
          margin: 0;
          font-size: 18px;
          font-weight: 600;
          color: var(--text-primary);
        }

        .editor-content {
          flex: 1;
          overflow-y: auto;
          padding: 24px;
        }

        .form-errors {
          margin-bottom: 16px;
          padding: 12px;
          background: rgba(243, 139, 168, 0.12);
          border: 1px solid var(--diff-delete);
          border-radius: 6px;
        }

        .error-message {
          margin: 0 0 4px 0;
          color: var(--diff-delete);
          font-size: 13px;
        }

        .error-message:last-child {
          margin-bottom: 0;
        }

        .form-group {
          margin-bottom: 16px;
        }

        .form-label {
          display: block;
          margin-bottom: 6px;
          font-size: 13px;
          font-weight: 500;
          color: var(--text-secondary);
        }

        .form-input,
        .form-textarea,
        .form-select {
          width: 100%;
          padding: 10px 12px;
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 6px;
          color: var(--text-primary);
          font-size: 14px;
          font-family: inherit;
          transition: all 0.15s;
          box-sizing: border-box;
        }

        .form-input:focus,
        .form-textarea:focus,
        .form-select:focus {
          outline: none;
          border-color: var(--accent-primary);
          box-shadow: 0 0 0 2px var(--accent-glow);
        }

        .form-input::placeholder,
        .form-textarea::placeholder {
          color: var(--text-muted);
        }

        .form-textarea {
          resize: vertical;
          min-height: 80px;
        }

        .template-textarea {
          font-family: 'JetBrains Mono', monospace;
          font-size: 13px;
        }

        .form-hint {
          margin: 6px 0 0 0;
          font-size: 12px;
          color: var(--text-muted);
        }

        .form-row {
          display: flex;
          gap: 16px;
        }

        .form-row .form-group {
          flex: 1;
        }

        .icon-input {
          width: 60px;
          text-align: center;
          font-size: 20px;
        }

        .tags-input {
          display: flex;
          flex-wrap: wrap;
          gap: 6px;
          padding: 8px;
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 6px;
          min-height: 40px;
        }

        .tags-input:focus-within {
          border-color: var(--accent-primary);
          box-shadow: 0 0 0 2px var(--accent-glow);
        }

        .tag-item {
          display: flex;
          align-items: center;
          gap: 4px;
          padding: 4px 8px;
          background: var(--bg-secondary);
          border-radius: 4px;
          color: var(--text-primary);
          font-size: 12px;
        }

        .tag-remove {
          background: transparent;
          border: none;
          color: var(--text-muted);
          cursor: pointer;
          padding: 0;
          font-size: 10px;
          line-height: 1;
        }

        .tag-remove:hover {
          color: var(--diff-delete);
        }

        .tag-input {
          flex: 1;
          min-width: 100px;
          background: transparent;
          border: none;
          outline: none;
          color: var(--text-primary);
          font-size: 12px;
          font-family: inherit;
        }

        .tag-input::placeholder {
          color: var(--text-muted);
        }

        .params-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 12px;
        }

        .add-param-btn {
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          color: var(--accent-primary);
          padding: 6px 10px;
          border-radius: 4px;
          cursor: pointer;
          font-size: 12px;
          transition: all 0.15s;
        }

        .add-param-btn:hover {
          background: var(--bg-hover);
        }

        .no-params {
          color: var(--text-muted);
          font-size: 13px;
          font-style: italic;
        }

        .params-list {
          display: flex;
          flex-direction: column;
          gap: 12px;
        }

        .param-item {
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 6px;
          padding: 12px;
        }

        .param-fields {
          display: flex;
          gap: 8px;
          align-items: center;
          margin-bottom: 8px;
        }

        .param-name {
          width: 100px;
        }

        .param-label {
          flex: 1;
        }

        .param-type {
          width: 100px;
        }

        .param-required {
          display: flex;
          align-items: center;
          gap: 4px;
          font-size: 12px;
          color: var(--text-secondary);
          cursor: pointer;
          white-space: nowrap;
        }

        .param-required input {
          accent-color: var(--accent-primary);
        }

        .remove-param-btn {
          background: transparent;
          border: none;
          color: var(--text-muted);
          cursor: pointer;
          padding: 4px 6px;
          border-radius: 4px;
          font-size: 12px;
        }

        .remove-param-btn:hover {
          background: var(--bg-hover);
          color: var(--diff-delete);
        }

        .param-placeholder {
          font-size: 12px;
        }

        .editor-footer {
          display: flex;
          justify-content: flex-end;
          gap: 12px;
          padding: 16px 24px;
          border-top: 1px solid var(--border-color);
        }

        .cancel-btn {
          padding: 10px 20px;
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 6px;
          color: var(--text-secondary);
          cursor: pointer;
          font-size: 14px;
          transition: all 0.15s;
        }

        .cancel-btn:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        .save-btn {
          padding: 10px 20px;
          background: var(--accent-primary);
          border: none;
          border-radius: 6px;
          color: var(--bg-primary);
          cursor: pointer;
          font-size: 14px;
          font-weight: 600;
          transition: all 0.15s;
        }

        .save-btn:hover {
          background: var(--accent-primary);
          box-shadow: 0 0 12px var(--accent-glow);
        }

        @media (max-width: 860px) {
          .library-main {
            grid-template-columns: 1fr;
          }

          .command-card-preview {
            border-left: 0;
            border-top: 1px solid var(--border-color);
            max-height: 38vh;
          }
        }
      `}</style>
    </div>
  );
}
