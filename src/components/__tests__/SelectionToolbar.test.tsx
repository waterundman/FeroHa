import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import SelectionToolbar from "../SelectionToolbar";

describe("SelectionToolbar", () => {
  it("does not expose the selection toolbar to accessibility when it is hidden", () => {
    const { queryByRole } = render(
      <SelectionToolbar visible={false} x={0} y={0} onAction={vi.fn()} onDismiss={vi.fn()} />,
    );

    expect(queryByRole("toolbar")).toBeNull();
  });
});
