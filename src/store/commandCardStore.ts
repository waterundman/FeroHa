/**
 * Command Card Store
 * FeroHa - Dual-Track AI Note IDE
 * Version: 2.1.8
 */

import { create } from "zustand";
import { persist } from "zustand/middleware";
import {
  type CommandCardDefinition,
  type CommandCardExport,
  type CommandCardFilter,
  type CommandCategory,
  generateCardId,
  validateCommandCard,
} from "../types/command-card";
import { commandCardRegistry } from "./commandCardRegistry";

// ============================================================================
// Store Types
// ============================================================================

interface CommandCardState {
  /** 用户自定义指令卡 */
  customCards: CommandCardDefinition[];
  /** 最近使用的指令卡ID */
  recentlyUsed: string[];
  /** 收藏的指令卡ID */
  favorites: string[];
  /** 指令卡使用统计 */
  usageStats: Record<string, number>;
  /** 当前活跃的筛选器 */
  activeFilter: CommandCardFilter;
  /** 搜索历史 */
  searchHistory: string[];
}

interface CommandCardActions {
  // CRUD操作
  addCard: (card: Omit<CommandCardDefinition["meta"], "id" | "createdAt" | "updatedAt"> & { prompt: CommandCardDefinition["prompt"]; params: CommandCardDefinition["params"] }) => boolean;
  updateCard: (id: string, updates: Partial<CommandCardDefinition>) => boolean;
  deleteCard: (id: string) => boolean;
  duplicateCard: (id: string) => boolean;

  // 查询操作
  getAllCards: (filter?: CommandCardFilter) => CommandCardDefinition[];
  getCardById: (id: string) => CommandCardDefinition | undefined;
  getCustomCards: () => CommandCardDefinition[];
  getBuiltinCards: () => CommandCardDefinition[];
  getCategories: () => CommandCategory[];
  getTags: () => string[];

  // 筛选和搜索
  setFilter: (filter: Partial<CommandCardFilter>) => void;
  clearFilter: () => void;
  addSearchHistory: (query: string) => void;
  clearSearchHistory: () => void;

  // 收藏和最近使用
  toggleFavorite: (id: string) => void;
  addToRecentlyUsed: (id: string) => void;
  recordUsage: (id: string) => void;

  // 导入导出
  exportCards: (customOnly?: boolean) => CommandCardExport;
  importCards: (data: CommandCardExport, overwrite?: boolean) => { success: number; failed: number; errors: string[] };
  exportToFile: (customOnly?: boolean) => void;
  importFromFile: (file: File) => Promise<{ success: number; failed: number; errors: string[] }>;

  // 分享
  shareCard: (id: string) => string;
  importFromShareLink: (link: string) => boolean;

  // 重置
  resetToDefault: () => void;
}

type CommandCardStore = CommandCardState & CommandCardActions;

// ============================================================================
// Constants
// ============================================================================

const STORAGE_KEY = "feroha-command-cards";
const MAX_RECENTLY_USED = 10;
const MAX_SEARCH_HISTORY = 20;

// ============================================================================
// Store Implementation
// ============================================================================

export const useCommandCardStore = create<CommandCardStore>()(
  persist(
    (set, get) => ({
      // ============================================================================
      // State
      // ============================================================================

      customCards: [],
      recentlyUsed: [],
      favorites: [],
      usageStats: {},
      activeFilter: {},
      searchHistory: [],

      // ============================================================================
      // CRUD Operations
      // ============================================================================

      addCard: (cardData) => {
        const id = generateCardId();
        const now = new Date().toISOString();

        const newCard: CommandCardDefinition = {
          meta: {
            id,
            ...cardData,
            version: "1.0.0",
            isCustom: true,
            createdAt: now,
            updatedAt: now,
          },
          prompt: cardData.prompt,
          params: cardData.params,
        };

        const errors = validateCommandCard(newCard);
        if (errors.length > 0) {
          console.error("Invalid command card:", errors);
          return false;
        }

        set((state) => ({
          customCards: [...state.customCards, newCard],
        }));

        return true;
      },

      updateCard: (id, updates) => {
        const { customCards } = get();
        const cardIndex = customCards.findIndex((c) => c.meta.id === id);

        if (cardIndex === -1) {
          // 尝试更新内置卡（不允许）
          if (commandCardRegistry.has(id)) {
            console.warn("Cannot update built-in command card:", id);
            return false;
          }
          return false;
        }

        const existingCard = customCards[cardIndex];
        const updatedCard: CommandCardDefinition = {
          ...existingCard,
          ...updates,
          meta: {
            ...existingCard.meta,
            ...updates.meta,
            id, // 确保ID不变
            updatedAt: new Date().toISOString(),
            version: incrementVersion(existingCard.meta.version),
          },
        };

        const errors = validateCommandCard(updatedCard);
        if (errors.length > 0) {
          console.error("Invalid command card update:", errors);
          return false;
        }

        const newCustomCards = [...customCards];
        newCustomCards[cardIndex] = updatedCard;

        set({ customCards: newCustomCards });
        return true;
      },

      deleteCard: (id) => {
        const { customCards } = get();
        const cardIndex = customCards.findIndex((c) => c.meta.id === id);

        if (cardIndex === -1) {
          console.warn("Card not found:", id);
          return false;
        }

        set((state) => ({
          customCards: state.customCards.filter((c) => c.meta.id !== id),
          favorites: state.favorites.filter((favId) => favId !== id),
          recentlyUsed: state.recentlyUsed.filter((recentId) => recentId !== id),
        }));

        return true;
      },

      duplicateCard: (id) => {
        const card = get().getCardById(id);
        if (!card) return false;

        const newId = generateCardId();
        const now = new Date().toISOString();

        const duplicatedCard: CommandCardDefinition = {
          ...card,
          meta: {
            ...card.meta,
            id: newId,
            name: `${card.meta.name} (Copy)`,
            version: "1.0.0",
            createdAt: now,
            updatedAt: now,
          },
        };

        set((state) => ({
          customCards: [...state.customCards, duplicatedCard],
        }));

        return true;
      },

      // ============================================================================
      // Query Operations
      // ============================================================================

      getAllCards: (filter) => {
        const { customCards, activeFilter } = get();
        const currentFilter = filter || activeFilter;

        // 获取内置卡
        const builtinCards = commandCardRegistry.getAll(currentFilter);

        // 合并自定义卡
        let allCards = [...builtinCards, ...customCards];

        // 应用筛选器
        if (currentFilter) {
          if (currentFilter.category && currentFilter.category !== "all") {
            allCards = allCards.filter((card) => card.meta.category === currentFilter.category);
          }

          if (currentFilter.tags && currentFilter.tags.length > 0) {
            allCards = allCards.filter((card) =>
              currentFilter.tags!.some((tag) => card.meta.tags.includes(tag))
            );
          }

          if (currentFilter.query) {
            const query = currentFilter.query.toLowerCase();
            allCards = allCards.filter(
              (card) =>
                card.meta.name.toLowerCase().includes(query) ||
                card.meta.description.toLowerCase().includes(query) ||
                card.meta.tags.some((tag) => tag.toLowerCase().includes(query))
            );
          }

          if (currentFilter.customOnly) {
            allCards = allCards.filter((card) => card.meta.isCustom);
          }

          if (currentFilter.sortBy) {
            allCards.sort((a, b) => {
              let comparison = 0;
              switch (currentFilter.sortBy) {
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
              return currentFilter.sortOrder === "desc" ? -comparison : comparison;
            });
          }
        }

        return allCards;
      },

      getCardById: (id) => {
        const { customCards } = get();
        const customCard = customCards.find((c) => c.meta.id === id);
        if (customCard) return customCard;

        return commandCardRegistry.get(id);
      },

      getCustomCards: () => {
        return get().customCards;
      },

      getBuiltinCards: () => {
        return commandCardRegistry.getAll();
      },

      getCategories: () => {
        const { customCards } = get();
        const builtinCategories = commandCardRegistry.getCategories();
        const customCategories = [...new Set(customCards.map((c) => c.meta.category))];
        return [...new Set([...builtinCategories, ...customCategories])];
      },

      getTags: () => {
        const { customCards } = get();
        const builtinTags = commandCardRegistry.getTags();
        const customTags = [...new Set(customCards.flatMap((c) => c.meta.tags))];
        return [...new Set([...builtinTags, ...customTags])];
      },

      // ============================================================================
      // Filter and Search
      // ============================================================================

      setFilter: (filter) => {
        set((state) => ({
          activeFilter: { ...state.activeFilter, ...filter },
        }));
      },

      clearFilter: () => {
        set({ activeFilter: {} });
      },

      addSearchHistory: (query) => {
        if (!query.trim()) return;

        set((state) => {
          const newHistory = [
            query,
            ...state.searchHistory.filter((q) => q !== query),
          ].slice(0, MAX_SEARCH_HISTORY);
          return { searchHistory: newHistory };
        });
      },

      clearSearchHistory: () => {
        set({ searchHistory: [] });
      },

      // ============================================================================
      // Favorites and Recently Used
      // ============================================================================

      toggleFavorite: (id) => {
        set((state) => {
          const isFavorite = state.favorites.includes(id);
          return {
            favorites: isFavorite
              ? state.favorites.filter((favId) => favId !== id)
              : [...state.favorites, id],
          };
        });
      },

      addToRecentlyUsed: (id) => {
        set((state) => {
          const newRecentlyUsed = [
            id,
            ...state.recentlyUsed.filter((recentId) => recentId !== id),
          ].slice(0, MAX_RECENTLY_USED);
          return { recentlyUsed: newRecentlyUsed };
        });
      },

      recordUsage: (id) => {
        set((state) => ({
          usageStats: {
            ...state.usageStats,
            [id]: (state.usageStats[id] || 0) + 1,
          },
        }));
        get().addToRecentlyUsed(id);
      },

      // ============================================================================
      // Import/Export
      // ============================================================================

      exportCards: (customOnly = false) => {
        const { customCards } = get();

        const cards = customOnly
          ? customCards
          : [...commandCardRegistry.getAll(), ...customCards];

        return {
          version: "2.1.8",
          exportedAt: new Date().toISOString(),
          cards,
        };
      },

      importCards: (data, overwrite = false) => {
        const result = { success: 0, failed: 0, errors: [] as string[] };

        for (const card of data.cards) {
          const { customCards } = get();
          const existingIndex = customCards.findIndex((c) => c.meta.id === card.meta.id);

          if (existingIndex !== -1 && !overwrite) {
            result.failed++;
            result.errors.push(`Card already exists: ${card.meta.id}`);
            continue;
          }

          const errors = validateCommandCard(card);
          if (errors.length > 0) {
            result.failed++;
            result.errors.push(`Invalid card ${card.meta.id}: ${errors.join(", ")}`);
            continue;
          }

          if (existingIndex !== -1) {
            // 覆盖现有卡
            const newCustomCards = [...customCards];
            newCustomCards[existingIndex] = card;
            set({ customCards: newCustomCards });
          } else {
            // 添加新卡
            set((state) => ({
              customCards: [...state.customCards, card],
            }));
          }

          result.success++;
        }

        return result;
      },

      exportToFile: (customOnly = false) => {
        const exportData = get().exportCards(customOnly);
        const blob = new Blob([JSON.stringify(exportData, null, 2)], {
          type: "application/json",
        });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `feroha-command-cards-${new Date().toISOString().split("T")[0]}.json`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      },

      importFromFile: async (file) => {
        try {
          const text = await file.text();
          const data = JSON.parse(text) as CommandCardExport;

          if (!data.version || !data.cards || !Array.isArray(data.cards)) {
            return {
              success: 0,
              failed: 0,
              errors: ["Invalid file format"],
            };
          }

          return get().importCards(data);
        } catch (error) {
          return {
            success: 0,
            failed: 0,
            errors: [`Failed to parse file: ${error}`],
          };
        }
      },

      // ============================================================================
      // Sharing
      // ============================================================================

      shareCard: (id) => {
        const card = get().getCardById(id);
        if (!card) return "";

        const shareData = {
          version: "2.1.8",
          cards: [card],
        };

        const encoded = btoa(JSON.stringify(shareData));
        return `feroha://import/${encoded}`;
      },

      importFromShareLink: (link) => {
        try {
          if (!link.startsWith("feroha://import/")) {
            return false;
          }

          const encoded = link.replace("feroha://import/", "");
          const decoded = atob(encoded);
          const data = JSON.parse(decoded) as CommandCardExport;

          const result = get().importCards(data);
          return result.success > 0;
        } catch (error) {
          console.error("Failed to import from share link:", error);
          return false;
        }
      },

      // ============================================================================
      // Reset
      // ============================================================================

      resetToDefault: () => {
        set({
          customCards: [],
          recentlyUsed: [],
          favorites: [],
          usageStats: {},
          searchHistory: [],
        });
      },
    }),
    {
      name: STORAGE_KEY,
      partialize: (state) => ({
        customCards: state.customCards,
        recentlyUsed: state.recentlyUsed,
        favorites: state.favorites,
        usageStats: state.usageStats,
        searchHistory: state.searchHistory,
      }),
    }
  )
);

// ============================================================================
// Helper Functions
// ============================================================================

function incrementVersion(version: string): string {
  const parts = version.split(".").map(Number);
  if (parts.length !== 3) return version;

  parts[2] += 1;
  return parts.join(".");
}

// ============================================================================
// Selectors
// ============================================================================

export const selectCustomCards = (state: CommandCardStore) => state.customCards;
export const selectFavorites = (state: CommandCardStore) => state.favorites;
export const selectRecentlyUsed = (state: CommandCardStore) => state.recentlyUsed;
export const selectUsageStats = (state: CommandCardStore) => state.usageStats;
export const selectActiveFilter = (state: CommandCardStore) => state.activeFilter;
export const selectSearchHistory = (state: CommandCardStore) => state.searchHistory;

export const selectFavoriteCards = (state: CommandCardStore) => {
  return state.favorites
    .map((id) => state.getCardById(id))
    .filter(Boolean) as CommandCardDefinition[];
};

export const selectRecentlyUsedCards = (state: CommandCardStore) => {
  return state.recentlyUsed
    .map((id) => state.getCardById(id))
    .filter(Boolean) as CommandCardDefinition[];
};

export const selectMostUsedCards = (state: CommandCardStore, limit = 5) => {
  return Object.entries(state.usageStats)
    .sort(([, a], [, b]) => b - a)
    .slice(0, limit)
    .map(([id]) => state.getCardById(id))
    .filter(Boolean) as CommandCardDefinition[];
};
