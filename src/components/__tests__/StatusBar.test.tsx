import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import StatusBar, { modeStatusLabel } from "../StatusBar";
import { useAppStore } from "../../hooks/useAppStore";

describe("StatusBar", () => {
  beforeEach(() => {
    useAppStore.setState({
      currentNote: { path: "notes/triad.md", content: "# AI 面三主体" },
      cursorLine: 12,
      cursorCol: 4,
      isDirty: false,
      saveStatus: "success",
      mode: "ai",
    });
  });

  it("uses semantic chips instead of pipe-separated debug text", () => {
    render(<StatusBar />);

    const bar = screen.getByRole("status");

    expect(bar.textContent).toContain("notes/triad.md");
    expect(screen.getByTitle("跳转到行").textContent).toBe("行 12，列 4");
    expect(screen.getByLabelText("保存状态").textContent).toBe("已保存");
    expect(screen.getByLabelText("当前面向").textContent).toBe("AI 面");
    expect(bar.textContent).not.toContain("|");
  });

  it("labels the two surfaces explicitly", () => {
    expect(modeStatusLabel("ai")).toBe("AI 面");
    expect(modeStatusLabel("human")).toBe("人类面");
  });
});
