import "../../i18n";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import AppLayout from "./AppLayout";

vi.mock("../sidebar/ConversationList", () => ({
  default: () => <div data-testid="conversation-sidebar" />,
}));

vi.mock("../projects/ProjectsSidebar", () => ({
  default: () => <div data-testid="projects-sidebar" />,
}));

describe("AppLayout", () => {
  it("renders the project sidebar on project routes and the conversation sidebar elsewhere", () => {
    const { rerender } = render(
      <MemoryRouter key="projects" initialEntries={["/projects"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/projects" element={<div>projects</div>} />
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId("projects-sidebar")).toBeInTheDocument();
    expect(screen.queryByTestId("conversation-sidebar")).not.toBeInTheDocument();

    rerender(
      <MemoryRouter key="generate" initialEntries={["/generate"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/projects" element={<div>projects</div>} />
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId("conversation-sidebar")).toBeInTheDocument();
  });

  it("collapses the conversation sidebar on settings routes", () => {
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/settings" element={<div>settings</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("conversation-sidebar")).not.toBeInTheDocument();
    expect(screen.getByText("settings")).toBeInTheDocument();
  });
});
