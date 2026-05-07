import { MemoryRouter } from "react-router-dom";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectsPage from "./ProjectsPage";

const getProjects = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("../lib/api", () => ({
  getProjects: (...args: unknown[]) => getProjects(...args),
}));

describe("ProjectsPage", () => {
  beforeEach(() => {
    getProjects.mockReset();
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
    expect(screen.getByText("86 images")).toBeInTheDocument();
    expect(screen.queryByText("Default Project")).not.toBeInTheDocument();
  });
});
