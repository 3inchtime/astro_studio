import { MemoryRouter, Route, Routes } from "react-router-dom";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectHomePage from "./ProjectHomePage";

const getProjects = vi.fn();
const getConversations = vi.fn();
const searchGenerations = vi.fn();
const setActiveConversationId = vi.fn();
const setActiveProjectId = vi.fn();
const navigate = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("../lib/api", () => ({
  getProjects: (...args: unknown[]) => getProjects(...args),
  getConversations: (...args: unknown[]) => getConversations(...args),
  searchGenerations: (...args: unknown[]) => searchGenerations(...args),
}));

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    setActiveConversationId,
    setActiveProjectId,
  }),
}));

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => navigate,
  };
});

describe("ProjectHomePage", () => {
  beforeEach(() => {
    getProjects.mockReset();
    getConversations.mockReset();
    searchGenerations.mockReset();
    navigate.mockReset();
    setActiveConversationId.mockReset();
    setActiveProjectId.mockReset();

    getProjects.mockResolvedValue([
      {
        id: "project-1",
        name: "Brand Storyboards",
        created_at: "",
        updated_at: "2026-05-07T01:00:00Z",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        conversation_count: 12,
        image_count: 86,
      },
    ]);
    getConversations.mockResolvedValue([
      {
        id: "conversation-1",
        project_id: "project-1",
        title: "Homepage hero direction",
        created_at: "",
        updated_at: "2026-05-07T01:00:00Z",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        generation_count: 8,
        latest_generation_at: "2026-05-07T01:00:00Z",
        latest_thumbnail: null,
      },
    ]);
    searchGenerations.mockResolvedValue({
      generations: [],
      total: 0,
      page: 1,
      page_size: 20,
    });
  });

  it("loads the project overview, recent conversations, and project-scoped images", async () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.getByText("projects.recentConversations")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "projects.manage" })).toBeInTheDocument();

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, "project-1");
    });

    expect(screen.getByText("86")).toBeInTheDocument();
  });
});
