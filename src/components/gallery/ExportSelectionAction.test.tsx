import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import ExportSelectionAction from "./ExportSelectionAction";
import { useExportStore } from "../../lib/exportStore";
it("counts every selected image and leaves recipe export opt-in", () => {
  const start = vi.fn();
  useExportStore.setState({ busy: false, start });
  render(
    <ExportSelectionAction
      rows={
        [
          { generation: { id: "a" }, images: [{ id: "1" }, { id: "2" }] },
          { generation: { id: "b" }, images: [{ id: "3" }] },
        ] as never
      }
      selectedIds={new Set(["a"])}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Export originals" }));
  expect(
    screen.getByText("1 generation record(s), 2 image(s)"),
  ).toBeInTheDocument();
  expect(screen.getByRole("checkbox")).not.toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Choose export folder" }));
  expect(start).toHaveBeenCalledWith(["a"], false);
});
