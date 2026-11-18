import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface CloseRequest {
  request_id: string;
  action: "close" | "exit" | "restart";
}

export function onCanvasCloseRequest(handler: (request: CloseRequest) => void) {
  return listen<CloseRequest>("canvas:close-requested", ({ payload }) => handler(payload));
}

export const enableCloseGuard = () => invoke<void>("enable_canvas_close_guard");
export const finishClose = (requestId: string) => invoke<void>("finish_canvas_close", { requestId });
export const cancelClose = (requestId: string) => invoke<void>("cancel_canvas_close", { requestId });
