import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import FavoriteButton from "./FavoriteButton";
import { useFavoriteFolders } from "../../hooks/useFavoriteFolders";

vi.mock("../../hooks/useFavoriteFolders", () => ({
  useFavoriteFolders: vi.fn(),
}));
const state = (loading = false, folderIds: string[] = []) => ({
  loading,
  folderIds,
  reload: vi.fn(),
  toggle: vi.fn(),
  setFolders: vi.fn(),
});
describe("FavoriteButton accessibility", () => {
  it("exposes the folder action as a named native button with favorite state", () => {
    vi.mocked(useFavoriteFolders).mockReturnValue(state(false, ["folder"]));
    const onClick = vi.fn();
    render(<FavoriteButton imageId="image" onClick={onClick} />);
    const button = screen.getByRole("button", { name: /folder/i });
    expect(button.tagName).toBe("BUTTON");
    expect(button).toHaveAttribute("aria-pressed", "true");
    button.focus();
    expect(button).toHaveFocus();
    fireEvent.click(button);
    expect(onClick).toHaveBeenCalledTimes(1);
  });
  it("retains its labeled target while folder state is loading", () => {
    vi.mocked(useFavoriteFolders).mockReturnValue(state(true));
    render(<FavoriteButton imageId="image" openSelector={vi.fn()} />);
    expect(screen.getByRole("button", { name: /folder/i })).toBeDisabled();
    expect(screen.getByRole("button")).toHaveAttribute("aria-busy", "true");
  });
  it("does not present a noninteractive status icon as a button", () => {
    vi.mocked(useFavoriteFolders).mockReturnValue(state(false, ["folder"]));
    render(<FavoriteButton imageId="image" />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(screen.getByRole("img")).toHaveAccessibleName();
  });
});
