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
