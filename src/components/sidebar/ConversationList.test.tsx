import { MemoryRouter } from "react-router-dom";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ConversationList from "./ConversationList";

const getConversations = vi.fn();
const getProjects = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("../../lib/api", () => ({
  getConversations: (...args: unknown[]) => getConversations(...args),
  getProjects: (...args: unknown[]) => getProjects(...args),
  archiveConversation: vi.fn(),
  createProject: vi.fn(),
  deleteConversation: vi.fn(),
  deleteProject: vi.fn(),
  moveConversationToProject: vi.fn(),
  pinConversation: vi.fn(),
  pinProject: vi.fn(),
  renameConversation: vi.fn(),
  renameProject: vi.fn(),
  toAssetUrl: (path: string) => path,
  unarchiveConversation: vi.fn(),
  unarchiveProject: vi.fn(),
  unpinConversation: vi.fn(),
  unpinProject: vi.fn(),
}));

describe("ConversationList", () => {
  beforeEach(() => {
    getProjects.mockReset();
    getConversations.mockReset();

    getProjects.mockResolvedValue([
      {
        id: "project-1",
        name: "Brand Storyboards",
        created_at: "",
        updated_at: "",
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
        updated_at: "",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        generation_count: 8,
        latest_generation_at: "",
        latest_thumbnail: null,
      },
    ]);
  });

  it("shows project context but no visible project strip when scoped", async () => {
    render(
      <MemoryRouter>
        <ConversationList
          activeProjectId="project-1"
          activeConversationId={null}
          refreshKey={0}
          onSelectProject={vi.fn()}
          onProjectCreated={vi.fn()}
          onSelectConversation={vi.fn()}
          onInitialConversation={vi.fn()}
          onNewConversation={vi.fn()}
        />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(getConversations).toHaveBeenCalledWith(undefined, "project-1", false);
    });

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.queryByText("sidebar.projects")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "sidebar.newProject" })).not.toBeInTheDocument();
  });
});
