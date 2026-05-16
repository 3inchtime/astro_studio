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
    present: previous,
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
    present: next,
    future,
  };
}
