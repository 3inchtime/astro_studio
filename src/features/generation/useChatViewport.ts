import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Message, PromptAgentMessage } from "../../types";

export function useChatViewport(messages: Message[], promptAgentMessages: PromptAgentMessage[]) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const autoScrollRef = useRef(true);
  const [chatViewportHeight, setChatViewportHeight] = useState(0);
  useLayoutEffect(() => {
    if (autoScrollRef.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, promptAgentMessages, chatViewportHeight]);

  useEffect(() => {
    const scrollElement = scrollRef.current;
    if (!scrollElement) return;

    const updateChatViewportHeight = () => {
      setChatViewportHeight(scrollElement.clientHeight);
    };

    updateChatViewportHeight();

    if (typeof ResizeObserver === "undefined") {
      return;
    }

    const resizeObserver = new ResizeObserver(() => {
      updateChatViewportHeight();
      if (autoScrollRef.current)
        scrollElement.scrollTop = scrollElement.scrollHeight;
    });
    resizeObserver.observe(scrollElement);
    if (scrollElement.firstElementChild)
      resizeObserver.observe(scrollElement.firstElementChild);

    return () => {
      resizeObserver.disconnect();
    };
  }, [messages.length, promptAgentMessages.length]);

  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    autoScrollRef.current = scrollHeight - scrollTop - clientHeight < 100;
  }, []);

  return { scrollRef, autoScrollRef, chatViewportHeight, handleScroll };
}
