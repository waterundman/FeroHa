import { describe, expect, it } from "vitest";
import { commandCardRegistry } from "../commandCardRegistry";

describe("commandCardRegistry built-in cards", () => {
  it("keeps the AI-face built-in command cards localized", () => {
    expect(commandCardRegistry.get("search")?.meta).toMatchObject({
      name: "搜索笔记",
      description: "按关键词或语义相似度检索笔记",
    });

    expect(commandCardRegistry.get("dream-cycle")?.meta).toMatchObject({
      name: "Dream 循环",
      description: "触发 NREM -> REM -> Insight 记忆整合",
    });

    expect(commandCardRegistry.get("graph-analysis")?.meta).toMatchObject({
      name: "图谱深潜",
      description: "围绕概念展开图谱邻域分析",
    });
  });

  it("localizes visible parameter labels and options", () => {
    const search = commandCardRegistry.get("search");
    const summarize = commandCardRegistry.get("summarize");

    expect(search?.params[0]).toMatchObject({
      label: "查询",
      placeholder: "输入搜索关键词...",
    });
    expect(search?.params[1]).toMatchObject({
      label: "结果数量",
      description: "返回结果数量",
    });
    expect(summarize?.params[1].options?.map((option) => option.label)).toEqual([
      "项目符号",
      "段落",
      "大纲",
    ]);
  });
});
