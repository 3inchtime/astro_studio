import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectChatPage from "./ProjectChatPage";

const navigate = vi.fn();
const setActiveProjectId = vi.fn();
const setActiveConversationId = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => navigate,
  };
});

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    setActiveProjectId,
    setActiveConversationId,
  }),
}));

vi.mock("./GeneratePage", () => ({
  default: () => <div>generate-page</div>,
}));

describe("ProjectChatPage", () => {
  beforeEach(() => {
    navigate.mockReset();
    setActiveProjectId.mockReset();
    setActiveConversationId.mockReset();
  });

  it("shows a clear back button that returns to the projects list", () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1/chat/conversation-1"]}>
        <Routes>
          <Route path="/projects/:projectId/chat/:conversationId" element={<ProjectChatPage />} />
        </Routes>
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "projects.backToList" }));

    expect(navigate).toHaveBeenCalledWith("/projects");
  });
});
