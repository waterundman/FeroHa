/**
 * Command Card Library
 * FeroHa - Dual-Track AI Note IDE
 * Version: 2.1.8
 */

import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import FeroHaIcon from "./FeroHaIcon";
import { useCommandCardStore } from "../store/commandCardStore";
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

  // State
  const [searchQuery, setSearchQuery] = useState("");
  const [activeCategory, setActiveCategory] = useState<CommandCategory | "all">("all");
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [sortField, setSortField] = useState<SortField>("name");
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">("asc");
  const [showCustomOnly, setShowCustomOnly] = useState(false);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);

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
      }
    },
    [mode, onSelect, onClose]
  );

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
              <div
                key={card.meta.id}
                className={`card-item ${favorites.includes(card.meta.id) ? "favorite" : ""} ${
                  card.meta.isCustom ? "custom" : ""
                }`}
                onClick={() => handleCardClick(card)}
              >
                <div className="card-header">
                  <span className="card-icon"><FeroHaIcon name={card.meta.icon} size={24} /></span>
                  <div className="card-actions">
                    <button
                      className={`action-btn favorite-btn ${
                        favorites.includes(card.meta.id) ? "active" : ""
                      }`}
                      onClick={(e) => {
                        e.stopPropagation();
                        handleToggleFavorite(card.meta.id);
                      }}
                      title={favorites.includes(card.meta.id) ? "取消收藏" : "收藏"}
                    >
                      <FeroHaIcon name="Star" size={14} />
                    </button>
                    {card.meta.isCustom && (
                      <>
                        <button
                          className="action-btn edit-btn"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleEditCard(card);
                          }}
                          title="编辑"
                        >
                          <FeroHaIcon name="Pencil" size={14} />
                        </button>
                        <button
                          className="action-btn delete-btn"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDeleteCard(card.meta.id);
                          }}
                          title="删除"
                        >
                          <FeroHaIcon name="X" size={14} />
                        </button>
                      </>
                    )}
                    <button
                      className="action-btn duplicate-btn"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDuplicateCard(card.meta.id);
                      }}
                      title="复制"
                    >
                      <FeroHaIcon name="Copy" size={14} />
                    </button>
                  </div>
                </div>

                <div className="card-body">
                  <h5 className="card-title">{card.meta.name}</h5>
                  <p className="card-description">{card.meta.description}</p>
                </div>

                <div className="card-footer">
                  <div className="card-tags">
                    {card.meta.tags.slice(0, 3).map((tag) => (
                      <span key={tag} className="card-tag">
                        {tag}
                      </span>
                    ))}
                    {card.meta.tags.length > 3 && (
                      <span className="card-tag-more">+{card.meta.tags.length - 3}</span>
                    )}
                  </div>
                  {card.meta.isCustom && <span className="custom-badge">自定义</span>}
                </div>

                <div className="card-meta">
                  <span className="meta-version">v{card.meta.version}</span>
                  {card.meta.author && <span className="meta-author">{card.meta.author}</span>}
                </div>
              </div>
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
    <div className="command-card-library">
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
          {renderCardGrid()}
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

        .library-overlay {
          position: absolute;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: rgba(0, 0, 0, 0.6);
          backdrop-filter: blur(4px);
        }

        .library-panel {
          position: relative;
          background: #1e1e2e;
          border-radius: 12px;
          width: 90%;
          max-width: 1200px;
          height: 85vh;
          display: flex;
          flex-direction: column;
          box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
          border: 1px solid #313244;
          animation: librarySlideIn 0.3s ease;
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
          border-bottom: 1px solid #313244;
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
          color: #cdd6f4;
        }

        .card-count {
          font-size: 14px;
          color: #6c7086;
          padding: 4px 8px;
          background: #313244;
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
          background: #89b4fa;
          color: #1e1e2e;
        }

        .create-btn:hover {
          background: #74c7ec;
        }

        .export-btn,
        .import-btn {
          background: #45475a;
          color: #cdd6f4;
        }

        .export-btn:hover,
        .import-btn:hover {
          background: #585b70;
        }

        .close-btn {
          background: transparent;
          border: none;
          color: #6c7086;
          font-size: 18px;
          cursor: pointer;
          padding: 8px;
          border-radius: 6px;
          transition: all 0.15s;
        }

        .close-btn:hover {
          background: #45475a;
          color: #cdd6f4;
        }

        .library-content {
          flex: 1;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        .search-section {
          padding: 16px 24px;
          border-bottom: 1px solid #313244;
        }

        .search-bar {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 12px 16px;
          background: #313244;
          border-radius: 8px;
          border: 1px solid #45475a;
          transition: all 0.15s;
        }

        .search-bar:focus-within {
          border-color: #89b4fa;
          box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.2);
        }

        .search-icon {
          color: #6c7086;
          font-size: 16px;
        }

        .search-input {
          flex: 1;
          background: transparent;
          border: none;
          outline: none;
          color: #cdd6f4;
          font-size: 14px;
          font-family: inherit;
        }

        .search-input::placeholder {
          color: #6c7086;
        }

        .clear-search {
          background: transparent;
          border: none;
          color: #6c7086;
          cursor: pointer;
          padding: 4px;
          border-radius: 4px;
          font-size: 12px;
        }

        .clear-search:hover {
          background: #45475a;
          color: #cdd6f4;
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
          color: #6c7086;
        }

        .clear-history {
          background: transparent;
          border: none;
          color: #6c7086;
          cursor: pointer;
          font-size: 12px;
          padding: 2px 6px;
          border-radius: 4px;
        }

        .clear-history:hover {
          background: #45475a;
          color: #cdd6f4;
        }

        .history-tags {
          display: flex;
          flex-wrap: wrap;
          gap: 6px;
        }

        .history-tag {
          background: #313244;
          border: none;
          color: #a6adc8;
          padding: 4px 8px;
          border-radius: 4px;
          cursor: pointer;
          font-size: 12px;
          transition: all 0.15s;
        }

        .history-tag:hover {
          background: #45475a;
          color: #cdd6f4;
        }

        .filters-section {
          padding: 12px 24px;
          border-bottom: 1px solid #313244;
          display: flex;
          justify-content: space-between;
          align-items: center;
          gap: 16px;
        }

        .category-tabs {
          display: flex;
          gap: 4px;
          overflow-x: auto;
        }

        .category-tab {
          background: transparent;
          border: none;
          color: #6c7086;
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
          background: #313244;
          color: #cdd6f4;
        }

        .category-tab.active {
          background: #45475a;
          color: #cdd6f4;
        }

        .tab-icon {
          font-size: 14px;
        }

        .filter-controls {
          display: flex;
          align-items: center;
          gap: 12px;
        }

        .view-toggle {
          display: flex;
          background: #313244;
          border-radius: 6px;
          overflow: hidden;
        }

        .view-btn {
          background: transparent;
          border: none;
          color: #6c7086;
          padding: 8px 12px;
          cursor: pointer;
          font-size: 14px;
          transition: all 0.15s;
        }

        .view-btn:hover {
          color: #cdd6f4;
        }

        .view-btn.active {
          background: #45475a;
          color: #cdd6f4;
        }

        .sort-select {
          background: #313244;
          border: 1px solid #45475a;
          color: #cdd6f4;
          padding: 8px 12px;
          border-radius: 6px;
          font-size: 13px;
          cursor: pointer;
        }

        .sort-select:focus {
          outline: none;
          border-color: #89b4fa;
        }

        .custom-only-toggle {
          display: flex;
          align-items: center;
          gap: 6px;
          color: #a6adc8;
          font-size: 13px;
          cursor: pointer;
        }

        .custom-only-toggle input {
          accent-color: #89b4fa;
        }

        .tags-section {
          padding: 12px 24px;
          border-bottom: 1px solid #313244;
        }

        .tags-scroll {
          display: flex;
          flex-wrap: wrap;
          gap: 6px;
        }

        .tag-btn {
          background: #313244;
          border: none;
          color: #a6adc8;
          padding: 6px 10px;
          border-radius: 4px;
          cursor: pointer;
          font-size: 12px;
          transition: all 0.15s;
        }

        .tag-btn:hover {
          background: #45475a;
          color: #cdd6f4;
        }

        .tag-btn.active {
          background: #89b4fa;
          color: #1e1e2e;
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
          color: #a6adc8;
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
          color: #6c7086;
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
          background: #313244;
          border-radius: 8px;
          padding: 16px;
          cursor: pointer;
          transition: all 0.2s ease;
          border: 2px solid transparent;
          position: relative;
        }

        .card-item:hover {
          background: #45475a;
          transform: translateY(-2px);
          box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }

        .card-item.favorite {
          border-color: #f9e2af;
        }

        .card-item.custom {
          border-left: 3px solid #a6e3a1;
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
          color: #6c7086;
          cursor: pointer;
          padding: 4px 6px;
          border-radius: 4px;
          font-size: 12px;
          transition: all 0.15s;
        }

        .action-btn:hover {
          background: #585b70;
          color: #cdd6f4;
        }

        .favorite-btn.active {
          color: #f9e2af;
        }

        .edit-btn:hover {
          color: #89b4fa;
        }

        .delete-btn:hover {
          color: #f38ba8;
        }

        .card-body {
          margin-bottom: 12px;
        }

        .card-title {
          margin: 0 0 6px 0;
          font-size: 15px;
          font-weight: 600;
          color: #cdd6f4;
        }

        .card-description {
          margin: 0;
          font-size: 12px;
          color: #a6adc8;
          line-height: 1.5;
          display: -webkit-box;
          -webkit-line-clamp: 2;
          -webkit-box-orient: vertical;
          overflow: hidden;
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
          background: #45475a;
          border-radius: 4px;
          color: #a6adc8;
        }

        .card-tag-more {
          font-size: 10px;
          padding: 2px 6px;
          color: #6c7086;
        }

        .custom-badge {
          font-size: 10px;
          padding: 2px 6px;
          background: #a6e3a1;
          border-radius: 4px;
          color: #1e1e2e;
          font-weight: 600;
        }

        .card-meta {
          display: flex;
          justify-content: space-between;
          align-items: center;
          font-size: 11px;
          color: #6c7086;
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
          color: #6c7086;
        }

        .no-results-icon {
          font-size: 48px;
          margin-bottom: 16px;
        }

        .no-results-text {
          margin: 0 0 8px 0;
          font-size: 16px;
          color: #a6adc8;
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
          background: #1e1e2e;
          border-radius: 12px;
          width: 90%;
          max-width: 600px;
          max-height: 90%;
          display: flex;
          flex-direction: column;
          box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
          border: 1px solid #313244;
        }

        .editor-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 16px 24px;
          border-bottom: 1px solid #313244;
        }

        .editor-header h3 {
          margin: 0;
          font-size: 18px;
          font-weight: 600;
          color: #cdd6f4;
        }

        .editor-content {
          flex: 1;
          overflow-y: auto;
          padding: 24px;
        }

        .form-errors {
          margin-bottom: 16px;
          padding: 12px;
          background: #f38ba820;
          border: 1px solid #f38ba8;
          border-radius: 6px;
        }

        .error-message {
          margin: 0 0 4px 0;
          color: #f38ba8;
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
          color: #a6adc8;
        }

        .form-input,
        .form-textarea,
        .form-select {
          width: 100%;
          padding: 10px 12px;
          background: #313244;
          border: 1px solid #45475a;
          border-radius: 6px;
          color: #cdd6f4;
          font-size: 14px;
          font-family: inherit;
          transition: all 0.15s;
          box-sizing: border-box;
        }

        .form-input:focus,
        .form-textarea:focus,
        .form-select:focus {
          outline: none;
          border-color: #89b4fa;
          box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.2);
        }

        .form-input::placeholder,
        .form-textarea::placeholder {
          color: #45475a;
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
          color: #6c7086;
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
          background: #313244;
          border: 1px solid #45475a;
          border-radius: 6px;
          min-height: 40px;
        }

        .tags-input:focus-within {
          border-color: #89b4fa;
          box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.2);
        }

        .tag-item {
          display: flex;
          align-items: center;
          gap: 4px;
          padding: 4px 8px;
          background: #45475a;
          border-radius: 4px;
          color: #cdd6f4;
          font-size: 12px;
        }

        .tag-remove {
          background: transparent;
          border: none;
          color: #6c7086;
          cursor: pointer;
          padding: 0;
          font-size: 10px;
          line-height: 1;
        }

        .tag-remove:hover {
          color: #f38ba8;
        }

        .tag-input {
          flex: 1;
          min-width: 100px;
          background: transparent;
          border: none;
          outline: none;
          color: #cdd6f4;
          font-size: 12px;
          font-family: inherit;
        }

        .tag-input::placeholder {
          color: #45475a;
        }

        .params-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 12px;
        }

        .add-param-btn {
          background: #45475a;
          border: none;
          color: #89b4fa;
          padding: 6px 10px;
          border-radius: 4px;
          cursor: pointer;
          font-size: 12px;
          transition: all 0.15s;
        }

        .add-param-btn:hover {
          background: #585b70;
        }

        .no-params {
          color: #6c7086;
          font-size: 13px;
          font-style: italic;
        }

        .params-list {
          display: flex;
          flex-direction: column;
          gap: 12px;
        }

        .param-item {
          background: #313244;
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
          color: #a6adc8;
          cursor: pointer;
          white-space: nowrap;
        }

        .param-required input {
          accent-color: #89b4fa;
        }

        .remove-param-btn {
          background: transparent;
          border: none;
          color: #6c7086;
          cursor: pointer;
          padding: 4px 6px;
          border-radius: 4px;
          font-size: 12px;
        }

        .remove-param-btn:hover {
          background: #45475a;
          color: #f38ba8;
        }

        .param-placeholder {
          font-size: 12px;
        }

        .editor-footer {
          display: flex;
          justify-content: flex-end;
          gap: 12px;
          padding: 16px 24px;
          border-top: 1px solid #313244;
        }

        .cancel-btn {
          padding: 10px 20px;
          background: #45475a;
          border: none;
          border-radius: 6px;
          color: #cdd6f4;
          cursor: pointer;
          font-size: 14px;
          transition: all 0.15s;
        }

        .cancel-btn:hover {
          background: #585b70;
        }

        .save-btn {
          padding: 10px 20px;
          background: #89b4fa;
          border: none;
          border-radius: 6px;
          color: #1e1e2e;
          cursor: pointer;
          font-size: 14px;
          font-weight: 600;
          transition: all 0.15s;
        }

        .save-btn:hover {
          background: #74c7ec;
        }
      `}</style>
    </div>
  );
}
