// A history entry can be revisited or remounted after a task receives its first
// conversation ID. A paid regenerate action belongs to that one navigation.
const handledStates = new WeakSet<object>();
const volatileEntries = new Set<string>();

export function consumeNavigationIntent(entry: string | undefined, state: object, action: "recipe" | "prompt") {
  // MemoryRouter/test callers can supply state without a history entry key.
  if (!entry) {
    if (handledStates.has(state)) return false;
    handledStates.add(state);
    return true;
  }
  const key = `astro:navigation:v1:${entry}:${action}`;
  if (volatileEntries.has(key)) return false;
  try {
    if (sessionStorage.getItem(key)) return false;
    sessionStorage.setItem(key, "1");
  } catch {
    volatileEntries.add(key);
  }
  return true;
}
