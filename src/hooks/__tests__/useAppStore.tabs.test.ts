import { beforeEach, describe, expect, it } from "vitest";
import { useAppStore } from "../useAppStore";

describe("useAppStore tab closing", () => {
  beforeEach(() => {
    useAppStore.setState({
      tabs: [],
      activeTabIndex: -1,
      closedTabs: [],
      currentNote: null,
      isDirty: false,
      splitActive: false,
      splitTabIndex: -1,
    });
  });

  it("leaves a clean empty editor state after closing the final tab", () => {
    useAppStore.setState({
      tabs: [
        {
          path: "Draft.md",
          title: "Draft",
          content: "# Draft",
          viewMode: "edit",
          isDirty: false,
        },
      ],
      activeTabIndex: 0,
      currentNote: { path: "Draft.md", content: "# Draft" },
      splitActive: true,
      splitTabIndex: 0,
    });

    useAppStore.getState().closeTab(0);

    expect(useAppStore.getState()).toMatchObject({
      tabs: [],
      activeTabIndex: -1,
      currentNote: null,
      isDirty: false,
      splitActive: false,
      splitTabIndex: -1,
    });
  });
});
