import { describe, expect, it } from "vitest";
import { modeDisplayLabel, modeToggleWrapperStyleForState } from "../ModeToggle";
import { emptyFileLabel, formatCursorLabel, modeStatusLabel, saveStatusLabel } from "../StatusBar";

describe("localized shell labels", () => {
  it("uses clear Chinese mode labels", () => {
    expect(modeDisplayLabel("ai")).toBe("AI 面");
    expect(modeDisplayLabel("human")).toBe("人类面");
    expect(modeStatusLabel("human")).toBe("人类面");
  });

  it("keeps the expanded mode label on its own navigation row", () => {
    expect(modeToggleWrapperStyleForState(false)).toMatchObject({
      width: "100%",
      minWidth: "100%",
    });
    expect(modeToggleWrapperStyleForState(true)).toMatchObject({
      width: "30px",
    });
  });

  it("uses compact Chinese status labels", () => {
    expect(emptyFileLabel).toBe("未打开文件");
    expect(formatCursorLabel(3, 9)).toBe("行 3，列 9");
    expect(saveStatusLabel("saving", false)).toBe("保存中");
    expect(saveStatusLabel("idle", true)).toBe("未保存");
  });
});
