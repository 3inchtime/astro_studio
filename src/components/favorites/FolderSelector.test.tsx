import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import FolderSelector from "./FolderSelector";
import PromptFolderSelector from "./PromptFolderSelector";

const api = vi.hoisted(() => ({
  getFolders: vi.fn(), getImageFolders: vi.fn(), createFolder: vi.fn(), addImageToFolders: vi.fn(), removeImageFromFolders: vi.fn(),
  getPromptFolders: vi.fn(), getPromptFavoriteFolders: vi.fn(), createPromptFolder: vi.fn(), addPromptFavoriteToFolders: vi.fn(), removePromptFavoriteFromFolders: vi.fn(),
  emit: vi.fn(),
}));
vi.mock("../../lib/api", () => api);
vi.mock("@tauri-apps/api/event", () => ({ emit: api.emit }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key }) }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { resolve, promise };
}

const variants = [
  { kind: "image", list: api.getFolders, load: api.getImageFolders, create: api.createFolder, add: api.addImageToFolders, remove: api.removeImageFromFolders, Dialog: ({ onClose }: { onClose: () => void }) => <FolderSelector imageId="item-1" onClose={onClose} /> },
  { kind: "prompt", list: api.getPromptFolders, load: api.getPromptFavoriteFolders, create: api.createPromptFolder, add: api.addPromptFavoriteToFolders, remove: api.removePromptFavoriteFromFolders, Dialog: ({ onClose }: { onClose: () => void }) => <PromptFolderSelector favoriteId="item-1" onClose={onClose} /> },
];

afterEach(cleanup);
describe.each(variants)("$kind folder dialog", ({ list, load, create, add, remove, Dialog }) => {
  beforeEach(() => {
    vi.resetAllMocks();
    list.mockResolvedValue([{ id: "a", name: "Folder A" }, { id: "b", name: "Folder B" }]);
    load.mockResolvedValue(["a"]);
    add.mockResolvedValue(undefined);
    remove.mockResolvedValue(undefined);
    api.emit.mockResolvedValue(undefined);
  });

  it("keeps one protected dialog through loading and restores focus on Escape", async () => {
    const pending = deferred<string[]>();
    load.mockReturnValue(pending.promise);
    function Harness() {
      const [open, setOpen] = useState(false);
      return <><button onClick={() => setOpen(true)}>Open picker</button>{open ? <Dialog onClose={() => setOpen(false)} /> : null}</>;
    }
    render(<Harness />);
    const opener = screen.getByRole("button", { name: "Open picker" });
    opener.focus(); fireEvent.click(opener);
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
    expect(screen.getByRole("button", { name: "favorites.confirm" })).toBeDisabled();
    await act(async () => pending.resolve(["a"]));
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(await screen.findByRole("checkbox", { name: "Folder A" })).toBeChecked();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it("distinguishes a failed load and retries before allowing edits", async () => {
    list.mockRejectedValueOnce(new Error("Offline"));
    render(<Dialog onClose={vi.fn()} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not load folders");
    expect(screen.getByRole("textbox", { name: "Folder name" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "favorites.confirm" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("checkbox", { name: "Folder A" })).toBeChecked();
    expect(screen.getByRole("button", { name: "favorites.confirm" })).toBeEnabled();
  });

  it("retries only unfinished membership changes after a partial save failure", async () => {
    remove.mockRejectedValueOnce(new Error("Offline"));
    const close = vi.fn();
    render(<Dialog onClose={close} />);
    fireEvent.click(await screen.findByRole("checkbox", { name: "Folder A" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Folder B" }));
    fireEvent.click(screen.getByRole("button", { name: "favorites.confirm" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not save folder changes");
    expect(close).not.toHaveBeenCalled();
    expect(screen.getByRole("checkbox", { name: "Folder B" })).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
    expect(add).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(2);
  });

  it("retains a failed new-folder name and retries creation", async () => {
    create.mockRejectedValueOnce(new Error("Offline")).mockResolvedValueOnce({ id: "new", name: "New folder" });
    render(<Dialog onClose={vi.fn()} />);
    await screen.findByRole("checkbox", { name: "Folder A" });
    const name = screen.getByRole("textbox", { name: "Folder name" });
    fireEvent.change(name, { target: { value: "New folder" } });
    fireEvent.click(screen.getByRole("button", { name: "Create folder" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not create folder");
    expect(name).toHaveValue("New folder");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("checkbox", { name: "New folder" })).toBeChecked();
    expect(name).toHaveValue("");
  });

  it("blocks duplicate saves and dismissal while a save is pending", async () => {
    const pending = deferred<void>();
    add.mockReturnValue(pending.promise);
    const close = vi.fn();
    render(<Dialog onClose={close} />);
    fireEvent.click(await screen.findByRole("checkbox", { name: "Folder B" }));
    const confirm = screen.getByRole("button", { name: "favorites.confirm" });
    fireEvent.click(confirm); fireEvent.click(confirm);
    expect(confirm).toBeDisabled();
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(close).not.toHaveBeenCalled();
    expect(add).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve());
    await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
  });
});
