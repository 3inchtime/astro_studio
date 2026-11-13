import { useState } from "react";

export function useCollectionSelection(ids: string[]) {
  const [stored, setStored] = useState(new Set<string>());
  const selectedIds = new Set(ids.filter((id) => stored.has(id)));
  return {
    selectedIds,
    selectLoaded: () => setStored(new Set(ids)),
    toggle: (id: string) => setStored((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else if (ids.includes(id)) next.add(id);
      return next;
    }),
    remove: (removed: string[]) => setStored((current) => new Set([...current].filter((id) => !removed.includes(id)))),
    clear: () => setStored(new Set()),
  };
}
