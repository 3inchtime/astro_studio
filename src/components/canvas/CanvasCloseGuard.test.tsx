import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import CanvasCloseGuard from "./CanvasCloseGuard";
import { queueCanvasSave, registerCanvasDraftFlusher, flushCanvasSaves } from "../../lib/canvas/persistence";
const native = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: native.listen }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
let listener: (event: { payload: { request_id: string; action: string } }) => void;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  native.invoke.mockReset().mockResolvedValue(undefined);
  native.listen.mockReset().mockImplementation(async (_name, callback) => { listener = callback; return () => {}; });
});
afterEach(async () => { cleanup(); await flushCanvasSaves(true); });
describe("native canvas close guard", () => {
  it.each(["close", "exit", "restart"])("waits for the mounted draft before approving %s", async (action) => {
    let resolve!: () => void;
    const disk = new Promise<void>((yes) => { resolve = yes; });
    const unregister = registerCanvasDraftFlusher(() => queueCanvasSave("guard", () => disk).promise);
    render(<CanvasCloseGuard />);
    await waitFor(() => expect(native.invoke).toHaveBeenCalledWith("enable_canvas_close_guard"));
    act(() => listener({ payload: { request_id: "request-1", action } }));
    expect(native.invoke).not.toHaveBeenCalledWith("finish_canvas_close", expect.anything());
    await act(async () => { resolve(); });
    await waitFor(() => expect(native.invoke).toHaveBeenCalledWith("finish_canvas_close", { requestId: "request-1" }));
    unregister();
  });
  it("keeps the app open on save failure and retries the same close request", async () => {
    const write = vi.fn().mockRejectedValueOnce(new Error("disk full")).mockResolvedValue(undefined);
    const unregister = registerCanvasDraftFlusher(() => queueCanvasSave("retry", write).promise);
    render(<CanvasCloseGuard />);
    await waitFor(() => expect(native.invoke).toHaveBeenCalledWith("enable_canvas_close_guard"));
    act(() => listener({ payload: { request_id: "retry-1", action: "close" } }));
    await screen.findByText("canvas.closeSaveError");
    expect(native.invoke).not.toHaveBeenCalledWith("finish_canvas_close", expect.anything());
    fireEvent.click(screen.getByRole("button", { name: "canvas.retrySaveAndClose" }));
    await waitFor(() => expect(native.invoke).toHaveBeenCalledWith("finish_canvas_close", { requestId: "retry-1" }));
    expect(write).toHaveBeenCalledTimes(2);
    unregister();
  });
  it("allows keeping the app open after a save failure without discarding the draft", async () => {
    const write = vi.fn().mockRejectedValueOnce(new Error("disk full")).mockResolvedValue(undefined);
    const unregister = registerCanvasDraftFlusher(() => queueCanvasSave("cancel", write).promise);
    render(<CanvasCloseGuard />);
    await waitFor(() => expect(native.invoke).toHaveBeenCalledWith("enable_canvas_close_guard"));
    act(() => listener({ payload: { request_id: "cancel-1", action: "exit" } }));
    fireEvent.click(await screen.findByRole("button", {name: "canvas.keepEditing"}));
    await waitFor(() => expect(native.invoke).toHaveBeenCalledWith("cancel_canvas_close", {requestId: "cancel-1"}));
    expect(write).toHaveBeenCalledTimes(1);
    unregister();
  });
});
