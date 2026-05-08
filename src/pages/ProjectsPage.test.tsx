import { MemoryRouter } from "react-router-dom";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectsPage from "./ProjectsPage";

const getProjects = vi.fn();
const createProject = vi.fn();
const searchGenerations = vi.fn();
const navigate = vi.fn();

vi.mock("react-i18next", () => {
  return {
    useTranslation: () => ({ t: (key: string) => key }),
  };
});

vi.mock("../lib/api", () => {
  return {
    getProjects: (...args: unknown[]) => getProjects(...args),
    createProject: (...args: unknown[]) => createProject(...args),
    searchGenerations: (...args: unknown[]) => searchGenerations(...args),
    pinProject: vi.fn(),
    unpinProject: vi.fn(),
    archiveProject: vi.fn(),
    deleteProject: vi.fn(),
    renameProject: vi.fn(),
    toAssetUrl: (p: string) => p,
  };
});

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => navigate,
  };
});

describe("ProjectsPage", () => {
  beforeEach(() => {
    getProjects.mockReset();
    createProject.mockReset();
    searchGenerations.mockReset();
    navigate.mockReset();
    searchGenerations.mockResolvedValue({
      generations: [],
      total: 0,
      page: 1,
      page_size: 5,
    });
    getProjects.mockResolvedValue([
      {
        id: "default",
        name: "Default Project",
        created_at: "",
        updated_at: "",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        conversation_count: 2,
        image_count: 5,
      },
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
  });

  it("shows user-facing projects and hides the default project", async () => {
    render(
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(getProjects).toHaveBeenCalledWith(false);
    });

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.queryByText("Default Project")).not.toBeInTheDocument();
  });

  it("opens the project dialog from the new project button", async () => {
    render(
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.newProject" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("projectDialog.createTitle")).toBeInTheDocument();
  });

  it("submits a trimmed project name to createProject", async () => {
    createProject.mockResolvedValue({ id: "project-created" });

    render(
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.newProject" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "  Launch Visuals  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.createSubmit" }));

    await waitFor(() => {
      expect(createProject).toHaveBeenCalledWith("Launch Visuals");
    });
  });

  it("navigates to the created project after successful creation", async () => {
    createProject.mockResolvedValue({ id: "project-created" });

    render(
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.newProject" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "Launch Visuals" },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.createSubmit" }));

    await waitFor(() => {
      expect(navigate).toHaveBeenCalledWith("/projects/project-created");
    });
  });

  it("displays a create error when project creation fails", async () => {
    createProject.mockRejectedValue(new Error("failed"));

    render(
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.newProject" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "Launch Visuals" },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.createSubmit" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("projectDialog.createError");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("renders an empty state when there are no user-facing projects", async () => {
    getProjects.mockResolvedValue([
      {
        id: "default",
        name: "Default Project",
        created_at: "",
        updated_at: "",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        conversation_count: 2,
        image_count: 5,
      },
    ]);

    render(
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    expect(await screen.findByText("projects.emptyTitle")).toBeInTheDocument();
    expect(screen.getByText("projects.emptyHint")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "sidebar.newProject" }).length).toBeGreaterThan(0);
  });

  it("renders a load error when projects fail to load", async () => {
    getProjects.mockRejectedValue(new Error("failed"));

    render(
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    expect(await screen.findByText("projects.loadError")).toBeInTheDocument();
  });

  it("renders a pinned marker for pinned projects", async () => {
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
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.getByText("projects.pinned")).toBeInTheDocument();
  });
});
