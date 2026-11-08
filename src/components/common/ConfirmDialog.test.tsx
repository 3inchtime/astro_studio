import { fireEvent, render, screen, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import ConfirmDialog from "./ConfirmDialog";

afterEach(cleanup);
const base = { title: "Delete image", confirmLabel: "Delete", cancelLabel: "Cancel", onConfirm: vi.fn() };

describe("confirmation keyboard and pending behavior", () => {
  it("focuses a safe action, traps focus and restores the trigger", () => {
    const trigger = document.createElement("button");
    document.body.append(trigger);
    trigger.focus();
    const onCancel = vi.fn();
    const view = render(<ConfirmDialog {...base} open onCancel={onCancel} />);
    const dialog = screen.getByRole("dialog");
    expect(dialog.contains(document.activeElement)).toBe(true);
    const buttons = dialog.querySelectorAll("button");
    buttons[buttons.length - 1].focus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(document.activeElement).toBe(buttons[0]);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();
    view.unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });

  it("does not dismiss a pending destructive action via backdrop or Escape", () => {
    const onCancel = vi.fn();
    render(<ConfirmDialog {...base} open loading onCancel={onCancel} />);
    fireEvent.click(screen.getByRole("dialog").parentElement!);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("Escape only dismisses the topmost dialog", () => {
    const outer = vi.fn();
    const inner = vi.fn();
    render(<><ConfirmDialog {...base} open onCancel={outer} /><ConfirmDialog {...base} title="Second" open onCancel={inner} /></>);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(inner).toHaveBeenCalledOnce();
    expect(outer).not.toHaveBeenCalled();
  });
});
