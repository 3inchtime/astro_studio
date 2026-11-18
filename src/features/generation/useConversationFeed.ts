import { useCallback, useEffect, useRef, useState, type SetStateAction } from "react";
import { getConversationGenerations, onGenerationJob, onGenerationPartial } from "../../lib/api";
import { applyGenerationPartial, generationsToMessages } from "../../lib/generationMessages";
import type { Message } from "../../types";

export function useConversationFeed(conversationId: string | null, refreshConversations: () => void) {
  const [messages, setMessages] = useState<Message[]>([]);
  const version = useRef(0);
  const request = useRef(0);
  const refreshSidebar = useRef(refreshConversations);
  refreshSidebar.current = refreshConversations;

  const updateMessages = useCallback((change: SetStateAction<Message[]>) => {
    version.current++;
    setMessages(change);
  }, []);

  const reload = useCallback(async () => {
    if (!conversationId) return;
    const id = ++request.current;
    const before = version.current;
    const rows = await getConversationGenerations(conversationId);
    if (request.current === id && version.current === before) {
      updateMessages(current => generationsToMessages(rows, current));
    }
  }, [conversationId, updateMessages]);

  useEffect(() => {
    let active = true;
    const jobs = onGenerationJob((job) => {
      if (!active || job.conversation_id !== conversationId || !conversationId) return;
      if (job.status === "running") return;
      void reload().catch(() => {});
      refreshSidebar.current();
    });
    const partials = onGenerationPartial((event) => {
      if (active && (!event.conversation_id || conversationId === event.conversation_id)) {
        setMessages((current) => applyGenerationPartial(current, event));
      }
    });
    // Register first so a fast worker cannot finish between the read and subscribe.
    void jobs.catch(() => {}).then(() => {
      if (active) void reload().catch(() => {});
    });
    return () => {
      active = false;
      request.current++;
      for (const listener of [jobs, partials]) void listener.then((release) => release()).catch(() => {});
    };
  }, [conversationId, reload]);

  const processing = messages.some((message) => message.status === "processing");
  useEffect(() => {
    if (!conversationId || !processing) return;
    const timer = window.setInterval(() => { void reload().catch(() => {}); }, 4000);
    return () => window.clearInterval(timer);
  }, [conversationId, processing, reload]);

  return { messages, updateMessages, isGenerating: processing };
}
