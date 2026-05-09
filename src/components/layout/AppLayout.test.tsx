import "../../i18n";
import { useEffect } from "react";
import { Link, MemoryRouter, Route, Routes } from "react-router-dom";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import AppLayout, { useLayoutContext } from "./AppLayout";

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
        })[key] ?? key,
    }),
  };
});

vi.mock("../sidebar/ConversationList", () => ({
  default: ({
    activeProjectId,
    activeConversationId,
  }: {
    activeProjectId: string | null;
    activeConversationId: string | null;
  }) => (
    <div data-testid="conversation-sidebar">
      <span data-testid="conversation-active-project">{activeProjectId ?? "none"}</span>
      <span data-testid="conversation-active-conversation">{activeConversationId ?? "none"}</span>
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

describe("AppLayout", () => {
  it("opens the rail theme picker and applies the selected preset", async () => {
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

    fireEvent.click(screen.getByRole("button", { name: "Open theme picker" }));

    expect(screen.getByRole("heading", { name: "Themes" })).toBeInTheDocument();
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
});
