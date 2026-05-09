import { MemoryRouter } from "react-router-dom";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProjectConversationPanel from "./ProjectConversationPanel";

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
  createConversation: vi.fn(),
  deleteConversation: vi.fn(),
  pinConversation: vi.fn(),
  renameConversation: vi.fn(),
  toAssetUrl: (path: string) => path,
  unpinConversation: vi.fn(),
}));

describe("ProjectConversationPanel", () => {
  it("renders larger conversation thumbnails in the project panel", async () => {
    const { container } = render(
      <MemoryRouter>
        <ProjectConversationPanel
          projectId="project-1"
          conversations={[
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
              latest_thumbnail: "/tmp/project-thumb.png",
            },
          ]}
          onConversationsChange={vi.fn()}
        />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Homepage hero direction")).toBeInTheDocument();

    const thumbnailImage = container.querySelector('img[src="/tmp/project-thumb.png"]');

    expect(thumbnailImage).not.toBeNull();
    expect(thumbnailImage?.parentElement).toHaveClass("h-[43px]", "w-[43px]");
  });
});
