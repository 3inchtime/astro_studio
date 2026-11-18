import { emit, listen } from "@tauri-apps/api/event";

const FAVORITES_CHANGED = "favorites:changed";

export function emitFavoritesChanged() {
  void emit(FAVORITES_CHANGED, {}).catch(() => {});
}

export function onFavoritesChanged(refresh: () => void) {
  return listen(FAVORITES_CHANGED, refresh);
}
