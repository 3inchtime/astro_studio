import { MemoryRouter, Route, Routes } from "react-router-dom";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ProjectHomePage from "./ProjectHomePage";

const getProjects = vi.fn();
const getConversations = vi.fn();
const searchGenerations = vi.fn();
const renameProject = vi.fn();
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
  renameProject: (...args: unknown[]) => renameProject(...args),
  archiveProject: vi.fn(),
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
  let promptSpy: { mockRestore: () => void };

  beforeEach(() => {
    getProjects.mockReset();
    getConversations.mockReset();
    searchGenerations.mockReset();
    renameProject.mockReset();
    navigate.mockReset();
    setActiveConversationId.mockReset();
    setActiveProjectId.mockReset();
    promptSpy = vi.spyOn(window, "prompt").mockReturnValue("Native Prompt Name");

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

  afterEach(() => {
    promptSpy.mockRestore();
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

  it("opens the rename dialog with the current project name", async () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.rename" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(screen.getByLabelText("projectDialog.nameLabel")).toHaveValue("Brand Storyboards");
    expect(promptSpy).not.toHaveBeenCalled();
  });

  it("submits a new name to renameProject", async () => {
    renameProject.mockResolvedValue(undefined);
    getProjects
      .mockResolvedValueOnce([
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
      ])
      .mockResolvedValueOnce([
        {
          id: "project-1",
          name: "New Name",
          created_at: "",
          updated_at: "2026-05-07T01:00:00Z",
          archived_at: null,
          pinned_at: null,
          deleted_at: null,
          conversation_count: 12,
          image_count: 86,
        },
      ]);

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.rename" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "New Name" },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.renameSubmit" }));

    await waitFor(() => {
      expect(renameProject).toHaveBeenCalledWith("project-1", "New Name");
    });
  });

  it("displays a rename error when rename fails", async () => {
    renameProject.mockRejectedValue(new Error("failed"));

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.rename" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "New Name" },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.renameSubmit" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("projectDialog.renameError");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("closes the rename dialog and updates optimistically when refresh fails after rename", async () => {
    const refreshError = new Error("refresh failed");
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    renameProject.mockResolvedValue(undefined);
    getProjects
      .mockResolvedValueOnce([
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
      ])
      .mockRejectedValueOnce(refreshError);

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.rename" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "New Name" },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.renameSubmit" }));

    await waitFor(() => {
      expect(renameProject).toHaveBeenCalledWith("project-1", "New Name");
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(screen.queryByText("projectDialog.renameError")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "New Name" })).toBeInTheDocument();
    expect(errorSpy).toHaveBeenCalledWith(refreshError);

    errorSpy.mockRestore();
  });
});
