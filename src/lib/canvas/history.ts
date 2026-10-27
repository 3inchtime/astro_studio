export interface CanvasHistory<T> {
  past: T[];
  present: T;
  future: T[];
}

export function createHistory<T>(initial: T): CanvasHistory<T> {
  return {
    past: [],
    present: initial,
    future: [],
  };
}

export function pushHistory<T>(
  history: CanvasHistory<T>,
  next: T,
  limit = 50,
): CanvasHistory<T> {
  if (Object.is(history.present, next)) {
    return history;
  }

  const past = [...history.past, history.present].slice(-limit);
  return {
    past,
    present: next,
    future: [],
  };
}

export function replaceHistory<T>(
  history: CanvasHistory<T>,
  next: T,
): CanvasHistory<T> {
  if (Object.is(history.present, next)) {
    return history;
  }

  return {
    ...history,
    present: next,
  };
}

export function undoHistory<T>(history: CanvasHistory<T>): CanvasHistory<T> {
  if (!history.past.length) {
    return history;
  }

  const previous = history.past[history.past.length - 1];
  return {
    past: history.past.slice(0, -1),
    present: preserveTransientState(previous, history.present),
    future: [history.present, ...history.future],
  };
}

export function redoHistory<T>(history: CanvasHistory<T>): CanvasHistory<T> {
  if (!history.future.length) {
    return history;
  }

  const [next, ...future] = history.future;
  return {
    past: [...history.past, history.present],
    present: preserveTransientState(next, history.present),
    future,
  };
}

function preserveTransientState<T>(snapshot: T, current: T): T {
  if (
    snapshot &&
    current &&
    typeof snapshot === "object" &&
    typeof current === "object" &&
    "viewport" in current
  ) {
    return {
      ...snapshot,
      viewport: current.viewport,
      ...("generation_requests" in current
        ? { generation_requests: current.generation_requests }
        : {}),
    };
  }
  return snapshot;
}
