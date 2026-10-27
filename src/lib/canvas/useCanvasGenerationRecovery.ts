import { useEffect, useRef } from "react";
import {
  getCanvasGenerationStatus,
  onGenerationComplete,
  onGenerationFailed,
  onGenerationProgress,
} from "../api";
import type { CanvasDocumentContent } from "../../types";
import { completeCanvasGeneration, failCanvasGeneration } from "./generation";

export function useCanvasGenerationRecovery(options: {
  documentId: string | null;
  enabled: boolean;
  getContent: () => CanvasDocumentContent;
  isCurrent: (id: string) => boolean;
  apply: (id: string, content: CanvasDocumentContent) => void;
  liveRequests: Set<string>;
}) {
  const latest = useRef(options);
  latest.current = options;
  useEffect(() => {
    if (!options.documentId || !options.enabled) return;
    const documentId = options.documentId;
    let disposed = false,
      running = false;
    async function reconcile() {
      if (running || disposed) return;
      running = true;
      try {
        const requests =
          latest.current
            .getContent()
            .generation_requests?.filter((r) => r.status === "pending") ?? [];
        for (const request of requests) {
          let result;
          try {
            result = await getCanvasGenerationStatus(
              documentId,
              request.request_id,
            );
          } catch {
            continue;
          }
          if (disposed || !latest.current.isCurrent(documentId)) return;
          const current = latest.current.getContent();
          let next = current;
          if (result?.status === "completed" || result?.status === "succeeded")
            next = completeCanvasGeneration(
              current,
              request.request_id,
              result.generation_id ?? request.generation_id ?? "",
              result.images,
            );
          else if (
            result &&
            ["failed", "cancelled", "interrupted"].includes(result.status)
          )
            next = failCanvasGeneration(
              current,
              request.request_id,
              result.error ?? "Generation failed",
            );
          else if (
            !result &&
            !latest.current.liveRequests.has(request.request_id) &&
            Date.now() - Date.parse(request.created_at) > 15000
          )
            next = failCanvasGeneration(
              current,
              request.request_id,
              "Generation interrupted before submission",
            );
          if (next !== current) latest.current.apply(documentId, next);
        }
      } finally {
        running = false;
      }
    }
    void reconcile();
    const timer = window.setInterval(() => void reconcile(), 2000);
    const subscriptions = [
      onGenerationComplete,
      onGenerationFailed,
      onGenerationProgress,
    ].map((subscribe) =>
      subscribe(() => void reconcile()).catch(() => () => {}),
    );
    return () => {
      disposed = true;
      window.clearInterval(timer);
      subscriptions.forEach((p) => void p.then((unlisten) => unlisten()));
    };
  }, [options.documentId, options.enabled]);
}
