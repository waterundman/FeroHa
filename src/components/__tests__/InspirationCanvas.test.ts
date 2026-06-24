import { describe, expect, it } from "vitest";
import {
  buildInspirationCanvasMarkdown,
  inspirationCanvasModeHint,
  inspirationCanvasToolTitle,
} from "../InspirationCanvas";
import type { CanvasCard, CanvasEdge } from "../../types/canvas";

describe("InspirationCanvas copy", () => {
  it("uses localized toolbar titles and mode hints", () => {
    expect(inspirationCanvasToolTitle("addNote")).toBe("添加笔记");
    expect(inspirationCanvasToolTitle("sticky")).toBe("新建便签");
    expect(inspirationCanvasModeHint("connect")).toBe("连接模式：从节点锚点拖到目标卡片");
    expect(inspirationCanvasModeHint("delete")).toBe("删除模式：点击卡片或连线删除");
    expect(inspirationCanvasModeHint("select")).toBeNull();
  });

  it("exports canvas markdown with localized headings", () => {
    const cards: CanvasCard[] = [
      card("a", "贝叶斯入口"),
      card("b", "证据更新"),
    ];
    const edges: CanvasEdge[] = [
      {
        id: "e1",
        fromCardId: "a",
        toCardId: "b",
        fromSide: "right",
        toSide: "left",
        label: "推导",
        color: "blue",
      },
    ];

    expect(buildInspirationCanvasMarkdown(cards, edges, "2026/6/1 13:40")).toContain(
      "# 灵感画布导出 - 2026/6/1 13:40\n\n## 笔记\n- [[贝叶斯入口]]\n- [[证据更新]]\n\n## 连接\n- [[贝叶斯入口]] --> [[证据更新]]（推导）\n",
    );
  });
});

function card(id: string, title: string): CanvasCard {
  return {
    id,
    title,
    notePath: `${title}.md`,
    preview: "",
    x: 0,
    y: 0,
    width: 240,
    height: 160,
    color: "blue",
  };
}
