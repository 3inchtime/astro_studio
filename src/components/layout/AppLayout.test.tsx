import "../../i18n";
import { useEffect } from "react";
import { Link, MemoryRouter, Route, Routes } from "react-router-dom";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AppLayout, { useLayoutContext } from "./AppLayout";

const checkForUpdate = vi.fn();
const isUpdateSupported = vi.fn();

vi.mock("react-i18next", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-i18next")>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) =>
        ({
          "nav.generate": "Generate",
          "nav.extract": "Image Prompt Extract",
          "nav.projects": "Projects",
          "nav.gallery": "Gallery",
          "nav.favorites": "Favorites",
          "nav.settings": "Settings",
          "theme.openPicker": "Open theme picker",
          "theme.title": "Themes",
          "theme.select": "Select {{name}} theme",
          "update.title": "App Update",
          "update.available": "New version available",
          "update.close": "Close",
          "update.later": "Remind Later",
          "update.download": "Download Update",
        })[key] ?? key,
    }),
  };
});

vi.mock("../../lib/api", () => ({
  checkForUpdate: () => checkForUpdate(),
  isUpdateSupported: () => isUpdateSupported(),
  createConversation: vi.fn(),
  installUpdate: vi.fn(),
  relaunchApp: vi.fn(),
}));

vi.mock("../sidebar/ConversationList", () => ({
  default: ({
    activeProjectId,
    activeConversationId,
    onClearActiveConversation,
  }: {
    activeProjectId: string | null;
    activeConversationId: string | null;
    onClearActiveConversation: () => void;
  }) => (
    <div data-testid="conversation-sidebar">
      <span data-testid="conversation-active-project">{activeProjectId ?? "none"}</span>
      <span data-testid="conversation-active-conversation">{activeConversationId ?? "none"}</span>
      <button type="button" onClick={onClearActiveConversation}>
        clear active conversation
      </button>
    </div>
  ),
}));

vi.mock("../projects/ProjectsSidebar", () => ({
  default: () => <div data-testid="projects-sidebar" />,
}));

function ProjectChatFixture() {
  const { setActiveProjectId, setActiveConversationId } = useLayoutContext();

  useEffect(() => {
    setActiveProjectId("project-1");
    setActiveConversationId("project-conversation-1");
  }, [setActiveConversationId, setActiveProjectId]);

  return <Link to="/generate">global conversations</Link>;
}

function CheckUpdateFixture() {
  const { checkForUpdates, updateSupported } = useLayoutContext();

  return (
    <button
      type="button"
      disabled={!updateSupported}
      onClick={() => void checkForUpdates({ silent: false })}
    >
      check updates
    </button>
  );
}

describe("AppLayout", () => {
  beforeEach(() => {
    checkForUpdate.mockReset();
    checkForUpdate.mockResolvedValue(null);
    isUpdateSupported.mockReset();
    isUpdateSupported.mockImplementation(() => new Promise(() => {}));
    vi.useRealTimers();
  });

  it("opens the rail theme picker and applies the selected preset", async () => {
    localStorage.clear();

    const { container } = render(
      <MemoryRouter initialEntries={["/generate"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(container.querySelector(".studio-app-shell")).toBeInTheDocument();
    expect(container.querySelector(".studio-nav-rail")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Open theme picker" }));

    expect(screen.getByRole("heading", { name: "Themes" })).toBeInTheDocument();
    expect(container.querySelector(".studio-floating-panel")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /Select .* theme/ })).toHaveLength(12);

    fireEvent.click(screen.getByRole("button", { name: "Select Ocean Depths theme" }));

    await waitFor(() => {
      expect(document.documentElement).toHaveAttribute("data-theme", "ocean-depths");
      expect(localStorage.getItem("astro-theme")).toBe("ocean-depths");
    });
  });

  it("uses the white preset as the default theme when nothing is stored", async () => {
    localStorage.clear();

    render(
      <MemoryRouter initialEntries={["/generate"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(document.documentElement).toHaveAttribute("data-theme", "pure-light");
      expect(localStorage.getItem("astro-theme")).toBe("pure-light");
    });
  });

  it("renders the project sidebar on project routes and the conversation sidebar elsewhere", () => {
    const { rerender } = render(
      <MemoryRouter key="projects" initialEntries={["/projects"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/projects" element={<div>projects</div>} />
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId("projects-sidebar")).toBeInTheDocument();
    expect(screen.queryByTestId("conversation-sidebar")).not.toBeInTheDocument();

    rerender(
      <MemoryRouter key="generate" initialEntries={["/generate"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/projects" element={<div>projects</div>} />
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId("conversation-sidebar")).toBeInTheDocument();
  });

  it("collapses the conversation sidebar on settings routes", () => {
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/settings" element={<div>settings</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("conversation-sidebar")).not.toBeInTheDocument();
    expect(screen.getByText("settings")).toBeInTheDocument();
  });

  it("collapses the conversation sidebar on gallery and favorites routes", () => {
    const { rerender } = render(
      <MemoryRouter key="gallery" initialEntries={["/gallery"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/gallery" element={<div>gallery</div>} />
            <Route path="/favorites" element={<div>favorites</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("conversation-sidebar")).not.toBeInTheDocument();
    expect(screen.getByText("gallery")).toBeInTheDocument();

    rerender(
      <MemoryRouter key="favorites" initialEntries={["/favorites"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/gallery" element={<div>gallery</div>} />
            <Route path="/favorites" element={<div>favorites</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("conversation-sidebar")).not.toBeInTheDocument();
    expect(screen.getByText("favorites")).toBeInTheDocument();
  });

  it("shows the extract nav item and collapses the conversation sidebar on extract routes", () => {
    render(
      <MemoryRouter initialEntries={["/extract"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/extract" element={<div>extract</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId("conversation-sidebar")).not.toBeInTheDocument();
    expect(screen.getByText("extract")).toBeInTheDocument();
    expect(screen.getByTitle("Image Prompt Extract")).toBeInTheDocument();
  });

  it("activates a passed conversation when entering generate from route state", async () => {
    render(
      <MemoryRouter
        initialEntries={[
          {
            pathname: "/generate",
            state: { activateConversationId: "extract-conversation-1" },
          } as never,
        ]}
      >
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("conversation-active-conversation")).toHaveTextContent(
        "extract-conversation-1",
      );
    });
  });

  it("clears a project conversation selection when returning to global conversations", async () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1/chat/project-conversation-1"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/projects/:projectId/chat/:conversationId" element={<ProjectChatFixture />} />
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("conversation-active-project")).toHaveTextContent("project-1");
      expect(screen.getByTestId("conversation-active-conversation")).toHaveTextContent("project-conversation-1");
    });

    fireEvent.click(screen.getByRole("link", { name: "global conversations" }));

    await waitFor(() => {
      expect(screen.getByTestId("conversation-active-project")).toHaveTextContent("none");
      expect(screen.getByTestId("conversation-active-conversation")).toHaveTextContent("none");
    });
    expect(screen.getByText("generate")).toBeInTheDocument();
  });

  it("keeps project chat context when clearing the active project conversation", async () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1/chat/project-conversation-1"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/projects/:projectId/chat/:conversationId" element={<ProjectChatFixture />} />
            <Route path="/projects/:projectId/chat" element={<div>project chat empty</div>} />
            <Route path="/projects/:projectId" element={<div>project home</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("conversation-active-conversation")).toHaveTextContent("project-conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "clear active conversation" }));

    await waitFor(() => {
      expect(screen.getByTestId("conversation-active-conversation")).toHaveTextContent("none");
    });
    expect(screen.getByText("project chat empty")).toBeInTheDocument();
    expect(screen.queryByText("project home")).not.toBeInTheDocument();
  });

  it("opens the update dialog when a child route manually checks and finds an update", async () => {
    isUpdateSupported.mockResolvedValue(true);
    checkForUpdate.mockResolvedValue({
      version: "0.0.24",
      current_version: "0.0.23",
      body: "Release notes",
      date: null,
    });

    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/settings" element={<CheckUpdateFixture />} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    const checkButton = screen.getByRole("button", { name: "check updates" });
    await waitFor(() => expect(checkButton).not.toBeDisabled());

    fireEvent.click(checkButton);

    expect(await screen.findByRole("dialog", { name: "App Update" })).toBeInTheDocument();
    expect(screen.getByText("Release notes")).toBeInTheDocument();
  });

  it("does not check for updates when updater support is disabled for the platform", async () => {
    vi.useFakeTimers();
    isUpdateSupported.mockResolvedValue(false);

    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/settings" element={<CheckUpdateFixture />} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    const checkButton = screen.getByRole("button", { name: "check updates" });

    await act(async () => {
      await Promise.resolve();
    });

    expect(isUpdateSupported).toHaveBeenCalled();
    expect(checkButton).toBeDisabled();

    fireEvent.click(checkButton);
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    expect(checkForUpdate).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog", { name: "App Update" })).not.toBeInTheDocument();
    vi.useRealTimers();
  });
});
