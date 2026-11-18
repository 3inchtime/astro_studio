import { listen } from "@tauri-apps/api/event";

interface OperationProgress {
  operation_id: string;
  completed: number;
  total: number;
}

export function onOperationProgress(id: string, update: (progress: OperationProgress) => void) {
  return listen<OperationProgress>("library:operation-progress", ({ payload }) => {
    if (payload.operation_id === id) update(payload);
  });
}
