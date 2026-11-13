interface PendingCanvasSave {
  promise: Promise<void>;
  failed: boolean;
  retry: () => PendingCanvasSave;
}
// Writes and reads share this queue across page mounts so a reopened document
// includes every accepted save, including work still rendering its preview.
const documentSaveQueues = new Map<string, PendingCanvasSave>();
export function queueCanvasSave(
  documentId: string,
  write: () => Promise<void>,
  previousPageSave = Promise.resolve(),
): PendingCanvasSave {
  const previous =
    documentSaveQueues.get(documentId)?.promise ?? Promise.resolve();
  const entry: PendingCanvasSave = {
    failed: false,
    promise: Promise.all([
      previous.catch(() => {}),
      previousPageSave.catch(() => {}),
    ]).then(write),
    retry: () => queueCanvasSave(documentId, write),
  };
  documentSaveQueues.set(documentId, entry);
  void entry.promise.then(
    () => {
      if (documentSaveQueues.get(documentId) === entry)
        documentSaveQueues.delete(documentId);
    },
    () => {
      entry.failed = true;
    },
  );
  return entry;
}
export async function waitForCanvasSaves(
  documentId: string,
  retryFailed: boolean,
  isCurrent: () => boolean,
) {
  let entry = documentSaveQueues.get(documentId);
  if (entry?.failed && retryFailed) entry = entry.retry();
  while (entry) {
    await entry.promise;
    if (!isCurrent()) return;
    const next = documentSaveQueues.get(documentId);
    if (next === entry) return;
    entry = next;
  }
}

const draftFlushers = new Set<() => Promise<void>>();
export function registerCanvasDraftFlusher(flush: () => Promise<void>) {
  draftFlushers.add(flush);
  return () => { draftFlushers.delete(flush); };
}

// Maintenance must not snapshot a library while accepted canvas saves are pending.
export async function flushCanvasSaves(retryFailed = false) {
  await Promise.all([...draftFlushers].map((flush) => flush()));
  if (retryFailed) {
    for (const entry of documentSaveQueues.values()) {
      if (entry.failed) entry.retry();
    }
  }
  while (documentSaveQueues.size) {
    const entries = [...documentSaveQueues.values()];
    await Promise.all(entries.map((entry) => entry.promise));
    if (entries.some((entry) => entry.failed)) throw new Error("Canvas changes could not be saved. Retry the canvas save first.");
  }
}

// Only call after the draft has been durably saved elsewhere.
export function acknowledgeCanvasSaveRecovery(documentId: string) {
  if (documentSaveQueues.get(documentId)?.failed) documentSaveQueues.delete(documentId);
}
