import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import type { Message, PromptAgentMessage } from "../../types";
import { useChatViewport } from "./useChatViewport";

const messages: Message[] = [];
const agentMessages: PromptAgentMessage[] = [];

function Feed({ visible }: { visible: boolean }) {
  const { scrollRef, handleScroll } = useChatViewport(messages, agentMessages, visible);
  return visible ? <div role="log" onScroll={handleScroll} ref={el => {
    if (el) Object.defineProperties(el, {
      scrollHeight: { configurable: true, value: 1000 },
      clientHeight: { configurable: true, value: 300 },
    });
    scrollRef.current = el;
  }} /> : null;
}

it("restores the reading position when returning from the node canvas", () => {
  const view = render(<Feed visible={false} />);
  view.rerender(<Feed visible />);
  const log = screen.getByRole("log");
  expect(log.scrollTop).toBe(1000);
  log.scrollTop = 250;
  fireEvent.scroll(log);
  view.rerender(<Feed visible={false} />);
  view.rerender(<Feed visible />);
  expect(screen.getByRole("log").scrollTop).toBe(250);
});
