import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export function hasTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return true;
  }

  return Boolean(
    (
      window as typeof window & {
        __TAURI_INTERNALS__?: { invoke?: unknown };
      }
    ).__TAURI_INTERNALS__?.invoke,
  );
}

export function toAssetUrl(filePath: string): string {
  if (!hasTauriRuntime()) {
    return filePath;
  }
  return convertFileSrc(filePath);
}

export function listenDesktopEvent<T>(event: string, handler: (data: T) => void) {
  if (!hasTauriRuntime()) return Promise.resolve(() => {});
  return listen(event, (e) => handler(e.payload as T));
}
