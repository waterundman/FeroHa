import { describe, expect, it } from "vitest";
import { styleForGraphEdge } from "../GraphView";

describe("GraphView edge styling", () => {
  it("uses dotted styling for related MDT edges", () => {
    expect(styleForGraphEdge("related")).toMatchObject({
      dash: [2, 5],
      width: 1,
      alpha: 0.55,
    });
  });

  it("uses a strong solid stroke for Dream bridge edges", () => {
    expect(styleForGraphEdge("bridge")).toMatchObject({
      dash: [],
      width: 2.2,
      alpha: 0.9,
    });
  });

  it("defaults missing edge types to reference styling", () => {
    expect(styleForGraphEdge(undefined)).toMatchObject({
      dash: [6, 4],
      width: 1.2,
      alpha: 0.7,
    });
  });
});
