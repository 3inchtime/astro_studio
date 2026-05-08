import { MemoryRouter, Route, Routes } from "react-router-dom";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ProjectHomePage from "./ProjectHomePage";

const getProjects = vi.fn();
const getConversations = vi.fn();
const searchGenerations = vi.fn();
const renameProject = vi.fn();
const pinProject = vi.fn();
const unpinProject = vi.fn();
const archiveProject = vi.fn();
const deleteProject = vi.fn();
const setActiveConversationId = vi.fn();
const setActiveProjectId = vi.fn();
const navigate = vi.fn();

function deferred<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, resolve, reject };
}

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    i18n: {
      language: "en",
      resolvedLanguage: "en",
    },
    t: (key: string) => key,
  }),
}));

vi.mock("../lib/api", () => ({
  getProjects: (...args: unknown[]) => getProjects(...args),
  getConversations: (...args: unknown[]) => getConversations(...args),
  searchGenerations: (...args: unknown[]) => searchGenerations(...args),
  renameProject: (...args: unknown[]) => renameProject(...args),
  pinProject: (...args: unknown[]) => pinProject(...args),
  unpinProject: (...args: unknown[]) => unpinProject(...args),
  archiveProject: (...args: unknown[]) => archiveProject(...args),
  deleteProject: (...args: unknown[]) => deleteProject(...args),
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
    pinProject.mockReset();
    unpinProject.mockReset();
    archiveProject.mockReset();
    deleteProject.mockReset();
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
    pinProject.mockResolvedValue(undefined);
    unpinProject.mockResolvedValue(undefined);
    archiveProject.mockResolvedValue(undefined);
    deleteProject.mockResolvedValue(undefined);
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
    expect(screen.getByRole("button", { name: "projects.manage" })).toBeInTheDocument();

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, "project-1");
    });

    expect(screen.getByText("projects.imageCountValue")).toBeInTheDocument();
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

  it("closes the rename dialog and updates optimistically", async () => {
    renameProject.mockResolvedValue(undefined);
    getProjects.mockResolvedValueOnce([
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
  });

  it("shows Unpin, not Pin, for pinned projects", async () => {
    getProjects.mockResolvedValue([
      {
        id: "project-1",
        name: "Brand Storyboards",
        created_at: "",
        updated_at: "2026-05-07T01:00:00Z",
        archived_at: null,
        pinned_at: "2026-05-07T01:00:00Z",
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

    expect(screen.getByRole("button", { name: "projects.unpin" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "projects.pin" })).not.toBeInTheDocument();
  });

  it("shows Pin, not Unpin, for unpinned projects", async () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));

    expect(screen.getByRole("button", { name: "projects.pin" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "projects.unpin" })).not.toBeInTheDocument();
  });

  it("pins an unpinned project optimistically", async () => {
    getProjects.mockResolvedValueOnce([
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

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "projects.pin" }));

    await waitFor(() => {
      expect(pinProject).toHaveBeenCalledWith("project-1");
    });
    expect(navigate).not.toHaveBeenCalledWith("/projects");
  });

  it("unpins a pinned project optimistically", async () => {
    getProjects.mockResolvedValueOnce([
      {
        id: "project-1",
        name: "Brand Storyboards",
        created_at: "",
        updated_at: "2026-05-07T01:00:00Z",
        archived_at: null,
        pinned_at: "2026-05-07T01:00:00Z",
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
    fireEvent.click(screen.getByRole("button", { name: "projects.unpin" }));

    await waitFor(() => {
      expect(unpinProject).toHaveBeenCalledWith("project-1");
    });
    expect(navigate).not.toHaveBeenCalledWith("/projects");
  });

  it("archives a project and navigates back to projects", async () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.archive" }));

    await waitFor(() => {
      expect(archiveProject).toHaveBeenCalledWith("project-1");
    });
    expect(navigate).toHaveBeenCalledWith("/projects");
  });

  it("requires confirmation before deleting a project", async () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.delete" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(deleteProject).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "projects.deleteConfirmAction" }));

    await waitFor(() => {
      expect(deleteProject).toHaveBeenCalledWith("project-1");
    });
    expect(navigate).toHaveBeenCalledWith("/projects");
  });

  it("shows an action error when pin fails", async () => {
    pinProject.mockRejectedValue(new Error("failed"));

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "projects.pin" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("projects.actionError");
    expect(navigate).not.toHaveBeenCalledWith("/projects");
  });

  it("shows an action error when unpin fails", async () => {
    getProjects.mockResolvedValue([
      {
        id: "project-1",
        name: "Brand Storyboards",
        created_at: "",
        updated_at: "2026-05-07T01:00:00Z",
        archived_at: null,
        pinned_at: "2026-05-07T01:00:00Z",
        deleted_at: null,
        conversation_count: 12,
        image_count: 86,
      },
    ]);
    unpinProject.mockRejectedValue(new Error("failed"));

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "projects.unpin" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("projects.actionError");
    expect(navigate).not.toHaveBeenCalledWith("/projects");
  });

  it("shows an action error when archive fails", async () => {
    archiveProject.mockRejectedValue(new Error("failed"));

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.archive" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("projects.actionError");
    expect(navigate).not.toHaveBeenCalledWith("/projects");
  });

  it("prevents repeated pin submits while pending", async () => {
    const pendingPin = deferred();
    pinProject.mockReturnValue(pendingPin.promise);

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "projects.pin" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "projects.pin" })).toBeDisabled();
    });
    fireEvent.click(screen.getByRole("button", { name: "projects.pin" }));

    expect(pinProject).toHaveBeenCalledTimes(1);
    await act(async () => {
      pendingPin.resolve(undefined);
      await pendingPin.promise;
    });
  });

  it("keeps delete confirmation open and shows a delete error when delete fails", async () => {
    deleteProject.mockRejectedValue(new Error("failed"));

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.delete" }));
    fireEvent.click(await screen.findByRole("button", { name: "projects.deleteConfirmAction" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("projects.deleteError");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(navigate).not.toHaveBeenCalledWith("/projects");
  });

  it("prevents repeated delete submits while pending", async () => {
    const pendingDelete = deferred();
    deleteProject.mockReturnValue(pendingDelete.promise);

    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "projects.manage" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.delete" }));
    fireEvent.click(await screen.findByRole("button", { name: "projects.deleteConfirmAction" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "projects.deleteConfirmAction" })).toBeDisabled();
    });
    fireEvent.click(screen.getByRole("button", { name: "projects.deleteConfirmAction" }));

    expect(deleteProject).toHaveBeenCalledTimes(1);
    await act(async () => {
      pendingDelete.resolve(undefined);
      await pendingDelete.promise;
    });
  });
});
