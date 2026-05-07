import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProjectActionsMenu from "./ProjectActionsMenu";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("ProjectActionsMenu", () => {
  it("renders no DOM when closed", () => {
    const { container } = render(
      <ProjectActionsMenu
        open={false}
        pinned={false}
        onRename={vi.fn()}
        onPin={vi.fn()}
        onUnpin={vi.fn()}
        onArchive={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(container).toBeEmptyDOMElement();
  });

  it("offers rename, pin, archive, and delete for unpinned projects", () => {
    const onRename = vi.fn();
    const onPin = vi.fn();
    const onArchive = vi.fn();
    const onDelete = vi.fn();

    render(
      <ProjectActionsMenu
        open
        pinned={false}
        onRename={onRename}
        onPin={onPin}
        onUnpin={vi.fn()}
        onArchive={onArchive}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.rename" }));
    fireEvent.click(screen.getByRole("button", { name: "projects.pin" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.archive" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.delete" }));

    expect(onRename).toHaveBeenCalledTimes(1);
    expect(onPin).toHaveBeenCalledTimes(1);
    expect(onArchive).toHaveBeenCalledTimes(1);
    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "projects.unpin" })).not.toBeInTheDocument();
  });

  it("offers unpin instead of pin for pinned projects", () => {
    const onUnpin = vi.fn();

    render(
      <ProjectActionsMenu
        open
        pinned
        onRename={vi.fn()}
        onPin={vi.fn()}
        onUnpin={onUnpin}
        onArchive={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "projects.unpin" }));

    expect(onUnpin).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "projects.pin" })).not.toBeInTheDocument();
  });
});
