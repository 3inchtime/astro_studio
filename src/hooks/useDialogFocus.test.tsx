import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState, type ReactNode } from "react";
import { useDialogFocus } from "./useDialogFocus";

function Dialog({ children, close = () => {}, busy = false, modal = true, name = "Dialog" }: { children: ReactNode; close?: () => void; busy?: boolean; modal?: boolean; name?: string }) {
  const ref = useDialogFocus({ open: true, onClose: close, busy, modal });
  return <div ref={ref} tabIndex={-1} role="dialog" aria-label={name}>{children}</div>;
}
afterEach(cleanup);

describe("useDialogFocus", () => {
  it("cycles mixed controls in their actual document order", () => {
    render(<Dialog><button>Close</button><input aria-label="Name" data-autofocus /><button>Cancel</button><button>Save</button></Dialog>);
    expect(screen.getByRole("textbox")).toHaveFocus();
    screen.getByRole("button", { name: "Save" }).focus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
    fireEvent.keyDown(document, { key: "Tab", shiftKey: true });
    expect(screen.getByRole("button", { name: "Save" })).toHaveFocus();
  });

  it("skips hidden ancestors, hidden inputs, negative tabindex and disabled autofocus", () => {
    render(<Dialog>
      <div style={{ display: "none" }}><button data-autofocus>Hidden first</button></div>
      <input type="hidden" />
      <button disabled data-autofocus>Disabled</button>
      <button tabIndex={-1}>Programmatic only</button>
      <button>Visible first</button><button>Visible last</button>
      <div style={{ display: "none" }}><button>Hidden last</button></div>
    </Dialog>);
    const first = screen.getByRole("button", { name: "Visible first" });
    expect(first).toHaveFocus();
    screen.getByRole("button", { name: "Visible last" }).focus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(first).toHaveFocus();
  });

  it("keeps initially nested dialogs in front for focus and Escape", () => {
    const outerClose = vi.fn(); const innerClose = vi.fn();
    render(<Dialog close={outerClose} name="Outer"><button>Outer control</button><Dialog close={innerClose} name="Inner"><button>Inner control</button></Dialog></Dialog>);
    expect(screen.getByRole("button", { name: "Inner control" })).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(innerClose).toHaveBeenCalledOnce();
    expect(outerClose).not.toHaveBeenCalled();
  });

  it("ignores Escape while composing text or awaiting an operation", () => {
    const close = vi.fn();
    const view = render(<Dialog close={close}><input aria-label="Name" /></Dialog>);
    fireEvent.keyDown(document, { key: "Escape", isComposing: true });
    fireEvent.keyDown(document, { key: "Escape", keyCode: 229 });
    expect(close).not.toHaveBeenCalled();
    view.rerender(<Dialog close={close} busy><input aria-label="Name" /></Dialog>);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).not.toHaveBeenCalled();
    view.rerender(<Dialog close={close}><input aria-label="Name" /></Dialog>);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
  });

  it("restores the parent trigger after closing a nested dialog", () => {
    const outerClose = vi.fn();
    function Nested() {
      const [innerOpen, setInnerOpen] = useState(false);
      return <Dialog close={outerClose} name="Outer"><button onClick={() => setInnerOpen(true)}>Open inner</button>{innerOpen ? <Dialog close={() => setInnerOpen(false)} name="Inner"><button>Inner action</button></Dialog> : null}</Dialog>;
    }
    render(<Nested />);
    const trigger = screen.getByRole("button", { name: "Open inner" });
    trigger.focus(); fireEvent.click(trigger);
    expect(screen.getByRole("button", { name: "Inner action" })).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Inner" })).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
    expect(outerClose).not.toHaveBeenCalled();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(outerClose).toHaveBeenCalledOnce();
  });

  it("does not capture focus or unrelated Escape in a nonmodal panel", () => {
    const close = vi.fn();
    render(<><input aria-label="Outside" /><Dialog close={close} modal={false}><button>Panel action</button></Dialog></>);
    const outside = screen.getByRole("textbox", { name: "Outside" });
    outside.focus();
    expect(outside).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).not.toHaveBeenCalled();
    screen.getByRole("button", { name: "Panel action" }).focus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
  });

  it("uses the panel when every control is disabled", () => {
    render(<Dialog busy><input disabled data-autofocus /><button disabled>Save</button></Dialog>);
    expect(screen.getByRole("dialog")).toHaveFocus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(screen.getByRole("dialog")).toHaveFocus();
  });
  it("wraps focus to a closed disclosure summary and skips its hidden contents", () => {
    render(<Dialog><button>Close</button><details><summary>Advanced</summary><button>Hidden advanced action</button></details></Dialog>);
    const close = screen.getByRole("button", { name: "Close" });
    close.focus(); fireEvent.keyDown(document, { key: "Tab", shiftKey: true });
    expect(screen.getByText("Advanced")).toHaveFocus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(close).toHaveFocus();
  });

});
