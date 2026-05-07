import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ProjectsSidebar from "./ProjectsSidebar";

const getProjects = vi.fn();
const createProject = vi.fn();
const onSelectProject = vi.fn();
const onProjectCreated = vi.fn();

vi.mock("react-i18next", () => {
  return {
    useTranslation: () => ({ t: (key: string) => key }),
  };
});

vi.mock("../../lib/api", () => {
  return {
    getProjects: (...args: unknown[]) => getProjects(...args),
    createProject: (...args: unknown[]) => createProject(...args),
  };
});

describe("ProjectsSidebar", () => {
  let promptSpy: { mockRestore: () => void };

  beforeEach(() => {
    getProjects.mockReset();
    createProject.mockReset();
    onSelectProject.mockReset();
    onProjectCreated.mockReset();
    promptSpy = vi.spyOn(window, "prompt").mockReturnValue("Native Prompt Name");
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

  afterEach(() => {
    promptSpy.mockRestore();
  });

  it("renders non-default projects returned by getProjects(false)", async () => {
    render(
      <ProjectsSidebar
        activeProjectId={null}
        onSelectProject={onSelectProject}
        onProjectCreated={onProjectCreated}
      />,
    );

    await waitFor(() => {
      expect(getProjects).toHaveBeenCalledWith(false);
    });

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.queryByText("Default Project")).not.toBeInTheDocument();
  });

  it("opens the project dialog from the plus button", async () => {
    render(
      <ProjectsSidebar
        activeProjectId={null}
        onSelectProject={onSelectProject}
        onProjectCreated={onProjectCreated}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.newProject" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(promptSpy).not.toHaveBeenCalled();
  });

  it("creates the project and calls onProjectCreated", async () => {
    createProject.mockResolvedValue({ id: "project-created" });

    render(
      <ProjectsSidebar
        activeProjectId={null}
        onSelectProject={onSelectProject}
        onProjectCreated={onProjectCreated}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.newProject" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "Launch Visuals" },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.createSubmit" }));

    await waitFor(() => {
      expect(createProject).toHaveBeenCalledWith("Launch Visuals");
      expect(onProjectCreated).toHaveBeenCalledWith("project-created");
    });
    expect(getProjects).toHaveBeenCalledTimes(2);
  });

  it("renders the error message when creation fails", async () => {
    createProject.mockRejectedValue(new Error("failed"));

    render(
      <ProjectsSidebar
        activeProjectId={null}
        onSelectProject={onSelectProject}
        onProjectCreated={onProjectCreated}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.newProject" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "Launch Visuals" },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.createSubmit" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("projectDialog.createError");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("keeps project creation successful when the refresh fails afterward", async () => {
    const loadError = new Error("refresh failed");
    createProject.mockResolvedValue({ id: "project-created" });
    getProjects.mockResolvedValueOnce([
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
    getProjects.mockRejectedValueOnce(loadError);
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <ProjectsSidebar
        activeProjectId={null}
        onSelectProject={onSelectProject}
        onProjectCreated={onProjectCreated}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "sidebar.newProject" }));
    fireEvent.change(await screen.findByLabelText("projectDialog.nameLabel"), {
      target: { value: "Launch Visuals" },
    });
    fireEvent.click(screen.getByRole("button", { name: "projectDialog.createSubmit" }));

    await waitFor(() => {
      expect(createProject).toHaveBeenCalledWith("Launch Visuals");
      expect(onProjectCreated).toHaveBeenCalledWith("project-created");
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByText("projectDialog.createError")).not.toBeInTheDocument();
    expect(errorSpy).toHaveBeenCalledWith(loadError);

    errorSpy.mockRestore();
  });
});
