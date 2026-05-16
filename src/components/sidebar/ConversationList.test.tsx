import { MemoryRouter } from "react-router-dom";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ConversationList from "./ConversationList";

const getConversations = vi.fn();
const getProjects = vi.fn();
const deleteConversation = vi.fn();
const navigate = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => navigate,
  };
});

vi.mock("../../lib/api", () => ({
  getConversations: (...args: unknown[]) => getConversations(...args),
  getProjects: (...args: unknown[]) => getProjects(...args),
  archiveConversation: vi.fn(),
  createProject: vi.fn(),
  deleteConversation: (...args: unknown[]) => deleteConversation(...args),
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
    deleteConversation.mockReset();
    navigate.mockReset();
    deleteConversation.mockResolvedValue(undefined);

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

  it("shows a back-to-projects button in the project-scoped sidebar header", async () => {
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
          onClearActiveConversation={vi.fn()}
          onNewConversation={vi.fn()}
        />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(getConversations).toHaveBeenCalledWith(undefined, "project-1", false);
    });

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "nav.projects" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "projects.backToList" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "sidebar.newProject" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "projects.backToList" }));

    expect(navigate).toHaveBeenCalledWith("/projects");
  });

  it("renders larger conversation thumbnails in the sidebar list", async () => {
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
        latest_thumbnail: "/tmp/sidebar-thumb.png",
      },
    ]);

    const { container } = render(
      <MemoryRouter>
        <ConversationList
          activeProjectId="project-1"
          activeConversationId={null}
          refreshKey={0}
          onSelectProject={vi.fn()}
          onProjectCreated={vi.fn()}
          onSelectConversation={vi.fn()}
          onInitialConversation={vi.fn()}
          onClearActiveConversation={vi.fn()}
          onNewConversation={vi.fn()}
        />
      </MemoryRouter>,
    );

    await screen.findByText("Homepage hero direction");

    const thumbnailImage = container.querySelector('img[src="/tmp/sidebar-thumb.png"]');

    expect(thumbnailImage).not.toBeNull();
    expect(thumbnailImage?.parentElement).toHaveClass("h-[43px]", "w-[43px]");
  });

  it("renders the active conversation as an elevated compact card", async () => {
    const { container } = render(
      <MemoryRouter>
        <ConversationList
          activeProjectId="project-1"
          activeConversationId="conversation-1"
          refreshKey={0}
          onSelectProject={vi.fn()}
          onProjectCreated={vi.fn()}
          onSelectConversation={vi.fn()}
          onInitialConversation={vi.fn()}
          onClearActiveConversation={vi.fn()}
          onNewConversation={vi.fn()}
        />
      </MemoryRouter>,
    );

    await screen.findByText("Homepage hero direction");

    const activeRow = container.querySelector("[data-active-conversation='true']");

    expect(activeRow).toHaveClass("bg-surface", "shadow-card", "ring-1");
  });

  it("opens an in-app confirmation dialog before deleting a conversation", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

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
          onClearActiveConversation={vi.fn()}
          onNewConversation={vi.fn()}
        />
      </MemoryRouter>,
    );

    await screen.findByText("Homepage hero direction");
    fireEvent.click(screen.getByRole("button", { name: "sidebar.conversationActions" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.delete" }));

    expect(confirmSpy).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toHaveTextContent("sidebar.deleteConversationConfirm");

    confirmSpy.mockRestore();
  });

  it("clears the active conversation without navigating to the project page after deleting it", async () => {
    const onSelectProject = vi.fn();
    const onClearActiveConversation = vi.fn();

    render(
      <MemoryRouter>
        <ConversationList
          activeProjectId="project-1"
          activeConversationId="conversation-1"
          refreshKey={0}
          onSelectProject={onSelectProject}
          onProjectCreated={vi.fn()}
          onSelectConversation={vi.fn()}
          onInitialConversation={vi.fn()}
          onClearActiveConversation={onClearActiveConversation}
          onNewConversation={vi.fn()}
        />
      </MemoryRouter>,
    );

    await screen.findByText("Homepage hero direction");
    fireEvent.click(screen.getByRole("button", { name: "sidebar.conversationActions" }));
    fireEvent.click(screen.getByRole("button", { name: "sidebar.delete" }));
    fireEvent.click(screen.getByRole("button", { name: "projects.deleteConfirmAction" }));

    await waitFor(() => {
      expect(deleteConversation).toHaveBeenCalledWith("conversation-1");
    });

    expect(onClearActiveConversation).toHaveBeenCalledTimes(1);
    expect(onSelectProject).not.toHaveBeenCalled();
  });
});
