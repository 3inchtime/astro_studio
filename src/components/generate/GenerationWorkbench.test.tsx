import { useState } from "react";
import { createPortal } from "react-dom";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import GenerationWorkbench from "./GenerationWorkbench";

let wide = false;
let resize: () => void;
beforeEach(() => {
  wide = false;
  vi.spyOn(window, "matchMedia").mockImplementation((query) => ({
    get matches() {
      return wide;
    },
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: (
      _type: string,
      listener: EventListenerOrEventListenerObject,
    ) => {
      resize = listener as () => void;
    },
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
});
function Draft() {
  const [value, setValue] = useState("original draft");
  return (
    <input
      aria-label="parameter draft"
      value={value}
      onChange={(e) => setValue(e.target.value)}
    />
  );
}
function Example() {
  return (
    <GenerationWorkbench>
      {(target) => (
        <>
          <input aria-label="prompt" />
          {target && createPortal(<Draft />, target)}
        </>
      )}
    </GenerationWorkbench>
  );
}
describe("GenerationWorkbench", () => {
  it("opens a focused parameter drawer and returns focus when Escape closes it", () => {
    render(<Example />);
    const trigger = screen.getByRole("button", {
      name: /^(生成参数|Generation parameters)$/i,
    });
    trigger.focus();
    fireEvent.click(trigger);
    expect(screen.getByRole("dialog")).toContainElement(
      document.activeElement as HTMLElement,
    );
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });
  it("keeps parameter drafts mounted when switching between drawer and docked panel", () => {
    render(<Example />);
    fireEvent.click(
      screen.getByRole("button", {
        name: /^(生成参数|Generation parameters)$/i,
      }),
    );
    const draft = screen.getByLabelText("parameter draft");
    fireEvent.change(draft, { target: { value: "keep this" } });
    act(() => {
      wide = true;
      resize();
    });
    expect(screen.getByLabelText("parameter draft")).toBe(draft);
    expect(draft).toHaveValue("keep this");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  it("keeps keyboard focus in parameters when a drawer becomes docked", () => {
    render(<Example />);
    fireEvent.click(
      screen.getByRole("button", {
        name: /^(生成参数|Generation parameters)$/i,
      }),
    );
    screen.getByRole("button", { name: "Close parameters" }).focus();
    act(() => {
      wide = true;
      resize();
    });
    expect(screen.getByLabelText("parameter draft")).toHaveFocus();
  });
});
