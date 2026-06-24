import { describe, expect, it } from "vitest";
import {
  filterGraphEdgesByCategory,
  filterGraphEdgesByViewMode,
  graphEdgeCategory,
  graphEdgeCategoryLabel,
  graphEdgeCategoryTitle,
  graphEdgeDisplayPlan,
  graphDisplaySummaryCounts,
  graphManagerCaption,
  graphMemoryRegion,
  graphMemoryLegendRegions,
  graphViewModeOptions,
  memoryRegionVisualPlan,
  styleForGraphEdge,
  type GraphEdgeCategory,
} from "../GraphView";

describe("GraphView edge styling", () => {
  it("uses dotted styling for related structure edges", () => {
    expect(styleForGraphEdge("related")).toMatchObject({
      dash: [2, 5],
      width: 1,
      alpha: 0.55,
      memoryRegion: "semantic",
    });
  });

  it("uses a strong solid stroke for Dream bridge overlay edges", () => {
    expect(styleForGraphEdge("bridge", "dream_bridge")).toMatchObject({
      dash: [],
      width: 2.2,
      alpha: 0.9,
      memoryRegion: "bridge",
    });
  });

  it("defaults missing edge types to reference styling", () => {
    expect(styleForGraphEdge(undefined)).toMatchObject({
      dash: [6, 4],
      width: 1.2,
      alpha: 0.7,
      memoryRegion: "working",
    });
  });
});

describe("GraphView edge filtering", () => {
  const enabled: Record<GraphEdgeCategory, boolean> = {
    wikilink: true,
    structure: true,
    dream: true,
  };

  it("classifies edges by source protocol", () => {
    expect(graphEdgeCategory({ from: "a", to: "b", origin: "wikilink" })).toBe("wikilink");
    expect(graphEdgeCategory({ from: "a", to: "b", origin: "frontmatter", edge_type: "parent" })).toBe("structure");
    expect(graphEdgeCategory({ from: "a", to: "b", origin: "jsonld", edge_type: "related" })).toBe("structure");
    expect(graphEdgeCategory({ from: "a", to: "b", origin: "dream", edge_type: "semantic" })).toBe("dream");
    expect(graphEdgeCategory({ from: "a", to: "b", edge_type: "bridge" })).toBe("dream");
  });

  it("filters wikilink, structure, and Dream edges independently", () => {
    const edges = [
      { from: "a", to: "b", origin: "wikilink", edge_type: "reference" as const },
      { from: "b", to: "c", origin: "frontmatter", edge_type: "parent" as const },
      { from: "c", to: "d", origin: "dream", edge_type: "bridge" as const },
    ];

    expect(filterGraphEdgesByCategory(edges, { ...enabled, dream: false })).toHaveLength(2);
    expect(filterGraphEdgesByCategory(edges, { ...enabled, structure: false }).map((edge) => edge.origin)).toEqual([
      "wikilink",
      "dream",
    ]);
    expect(filterGraphEdgesByCategory(edges, { wikilink: false, structure: false, dream: true })).toEqual([
      edges[2],
    ]);
  });

  it("uses demo edges only when the graph has no real edges", () => {
    const edges = [
      { from: "a", to: "b", origin: "wikilink", edge_type: "reference" as const },
    ];

    expect(
      graphEdgeDisplayPlan(edges, { wikilink: false, structure: false, dream: false }),
    ).toEqual({ edges: [], useDemo: false });
    expect(graphEdgeDisplayPlan([], enabled)).toEqual({ edges: [], useDemo: true });
  });
});

describe("GraphView Dream memory regions", () => {
  it("treats AI Manager as the owner of a Dream three-zone graph, not a separate triad view", () => {
    expect(graphManagerCaption()).toContain("AI Manager");
    expect(graphViewModeOptions().map((mode) => mode.value)).toEqual(["focus", "three-zone"]);
    expect(graphViewModeOptions().map((mode) => mode.value)).not.toContain("ai-triad");
  });

  it("keeps the visible legend focused on Dream memory partitions plus cross-zone links", () => {
    expect(graphMemoryLegendRegions()).toEqual(["working", "semantic", "long_term", "bridge"]);
    expect(memoryRegionVisualPlan("working").role).toBe("memory_zone");
    expect(memoryRegionVisualPlan("semantic").role).toBe("memory_zone");
    expect(memoryRegionVisualPlan("long_term").role).toBe("memory_zone");
    expect(memoryRegionVisualPlan("bridge").role).toBe("cross_zone_link");
  });

  it("maps Dream memory regions to the three-zone protocol plus bridge overlay", () => {
    expect(graphMemoryRegion({ memory_region: "working" })).toBe("working");
    expect(graphMemoryRegion({ memory_region: "semantic" })).toBe("semantic");
    expect(graphMemoryRegion({ memory_region: "long_term" })).toBe("long_term");
    expect(graphMemoryRegion({ memory_region: "dream_bridge" })).toBe("bridge");
  });

  it("maps edge origins into Dream zones", () => {
    expect(graphMemoryRegion({ origin: "jsonld", edge_type: "related" })).toBe("semantic");
    expect(graphMemoryRegion({ origin: "wikilink", edge_type: "reference" })).toBe("working");
    expect(graphMemoryRegion({ origin: "archive", edge_type: "source" })).toBe("long_term");
    expect(graphMemoryRegion({ origin: "dream", edge_type: "temporal" })).toBe("working");
    expect(graphMemoryRegion({ origin: "dream", edge_type: "semantic" })).toBe("semantic");
  });

  it("assigns distinct colors and motion speeds to memory regions", () => {
    expect(memoryRegionVisualPlan("working")).toMatchObject({
      label: "工作记忆",
      animationSpeed: 0.65,
    });
    expect(memoryRegionVisualPlan("semantic")).toMatchObject({
      label: "结构语义",
      animationSpeed: 0.22,
    });
    expect(memoryRegionVisualPlan("long_term")).toMatchObject({
      label: "长期记忆",
      animationSpeed: 0.08,
    });
    expect(memoryRegionVisualPlan("bridge")).toMatchObject({
      label: "跨区连接",
      animationSpeed: 1.05,
    });
  });

  it("focus mode keeps direct neighbors and bridge explanations", () => {
    const edges = [
      { from: "focus", to: "a", edge_type: "reference" as const },
      { from: "b", to: "c", edge_type: "reference" as const },
      { from: "x", to: "y", edge_type: "bridge" as const },
    ];

    expect(filterGraphEdgesByViewMode(edges, "focus", "focus")).toEqual([edges[0], edges[2]]);
    expect(filterGraphEdgesByViewMode(edges, "three-zone", "focus")).toEqual(edges);
  });
});

describe("GraphView UI copy", () => {
  it("reports demo graph counts when the canvas is showing the empty-state preview graph", () => {
    expect(graphDisplaySummaryCounts({ nodes: [], edges: [] })).toEqual({
      nodes: 5,
      edges: 4,
    });
  });

  it("uses localized labels for the three memory edge categories", () => {
    expect(graphEdgeCategoryLabel("wikilink")).toBe("Wiki 双链");
    expect(graphEdgeCategoryLabel("structure")).toBe("结构语义");
    expect(graphEdgeCategoryLabel("dream")).toBe("Dream 记忆");
  });

  it("uses localized toggle titles instead of raw category names", () => {
    expect(graphEdgeCategoryTitle("wikilink", true)).toBe("隐藏 Wiki 双链连接");
    expect(graphEdgeCategoryTitle("dream", false)).toBe("显示 Dream 记忆连接");
  });
});
