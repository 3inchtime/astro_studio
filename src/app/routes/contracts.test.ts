import { describe, expect, it } from "vitest";

import {
  isGenerateRoute,
  isProjectsRoute,
  isProjectChatRoute,
  isProjectHomeRoute,
  getRouteContext,
  isHideSidebarRoute,
  getRouteLayout,
  parseProjectIdFromPath,
  shouldHideSidebarForContext,
} from "./contracts";

describe("routes/contracts", () => {
  it("classifies static routes", () => {
    const generateContext = getRouteContext("/generate");
    expect(generateContext).toEqual({
      kind: "generate",
      pathname: "/generate",
      projectId: null,
      conversationId: null,
    });

    expect(getRouteContext("/projects").kind).toBe("projects");
    expect(getRouteContext("/gallery").kind).toBe("gallery");
    expect(getRouteContext("/favorites").kind).toBe("favorites");
    expect(getRouteContext("/settings").kind).toBe("settings");
    expect(getRouteContext("/generate/").kind).toBe("generate");
    expect(getRouteContext("/extract?x=1").kind).toBe("extract");
    expect(getRouteContext("/unknown-route").kind).toBe("unknown");
  });

  it("parses project route kinds and IDs", () => {
    expect(getRouteContext("/projects/proj-1")).toEqual({
      kind: "projectHome",
      pathname: "/projects/proj-1",
      projectId: "proj-1",
      conversationId: null,
    });

    expect(getRouteContext("/projects/proj-1/chat")).toEqual({
      kind: "chat",
      pathname: "/projects/proj-1/chat",
      projectId: "proj-1",
      conversationId: null,
    });

    expect(getRouteContext("/projects/proj%2D1/chat/chat%2Fconv%20x")).toEqual({
      kind: "chat",
      pathname: "/projects/proj%2D1/chat/chat%2Fconv%20x",
      projectId: "proj-1",
      conversationId: "chat/conv x",
    });

    expect(getRouteContext("/projects/proj-2/chat").kind).toBe("chat");
    expect(
      getRouteContext("/projects/proj-2/chat").conversationId,
    ).toBeNull();
    expect(getRouteContext("/projects/proj-3/chat/")).toEqual({
      kind: "chat",
      pathname: "/projects/proj-3/chat",
      conversationId: null,
      projectId: "proj-3",
    });
    expect(getRouteContext("/projects/a%2Fb/chat")).toEqual({
      kind: "chat",
      pathname: "/projects/a%2Fb/chat",
      conversationId: null,
      projectId: "a/b",
    });
    expect(getRouteContext("/projects/a%2Fb").pathname).toBe("/projects/a%2Fb");
    expect(getRouteContext("/projects/a%2Fb").projectId).toBe("a/b");
    expect(getRouteContext("/projects//proj-4//chat/?source=web")).toEqual({
      kind: "chat",
      pathname: "/projects/proj-4/chat",
      conversationId: null,
      projectId: "proj-4",
    });
  });

  it("protects hide-sidebar rules behind route context", () => {
    const extract = getRouteContext("/extract");
    const gallery = getRouteContext("/gallery");
    const projects = getRouteContext("/projects");
    const trash = getRouteContext("/trash");
    const favorites = getRouteContext("/favorites");
    const settings = getRouteContext("/settings");

    expect(isHideSidebarRoute("/extract")).toBe(true);
    expect(shouldHideSidebarForContext(extract)).toBe(true);
    expect(shouldHideSidebarForContext(gallery)).toBe(true);
    expect(shouldHideSidebarForContext(projects)).toBe(false);
    expect(shouldHideSidebarForContext(trash)).toBe(false);
    expect(shouldHideSidebarForContext(favorites)).toBe(true);
    expect(shouldHideSidebarForContext(settings)).toBe(true);
  });

  it("classifies route helpers consistently", () => {
    expect(isGenerateRoute("/generate")).toBe(true);
    expect(isGenerateRoute("/generate?tab=1")).toBe(true);
    expect(isProjectsRoute("/projects")).toBe(true);
    expect(isProjectsRoute("/projects/project-a")).toBe(false);

    expect(isProjectHomeRoute("/projects/project-a")).toBe(true);
    expect(isProjectHomeRoute("/projects/project-a/chat")).toBe(false);

    expect(isProjectChatRoute("/projects/project-a/chat")).toBe(true);
    expect(isProjectChatRoute("/projects/project-a/chat/convo-1")).toBe(true);
    expect(isProjectChatRoute("/projects/project-a")).toBe(false);
    expect(isProjectChatRoute("/projects/project-a/chat/convo/a")).toBe(false);
  });

  it("exposes stable route layout metadata", () => {
    expect(getRouteLayout("generate")).toEqual({
      kind: "generate",
      sidebar: { type: "conversations", useProjectFromPath: false },
      clearProjectOnEnter: true,
    });

    expect(getRouteLayout("projectHome")).toEqual({
      kind: "projectHome",
      sidebar: { type: "conversations", useProjectFromPath: true },
      clearProjectOnEnter: false,
    });

    expect(getRouteLayout("chat")).toEqual({
      kind: "chat",
      sidebar: { type: "conversations", useProjectFromPath: true },
      clearProjectOnEnter: false,
    });

    expect(getRouteLayout("gallery")).toEqual({
      kind: "gallery",
      sidebar: { type: "hidden", useProjectFromPath: false },
      clearProjectOnEnter: true,
    });

    expect(getRouteLayout("trash")).toEqual({
      kind: "trash",
      sidebar: { type: "conversations", useProjectFromPath: false },
      clearProjectOnEnter: true,
    });

    expect(getRouteLayout("unknown")).toEqual(getRouteLayout("default"));
  });

  it("extracts project id from project routes only", () => {
    expect(parseProjectIdFromPath("/projects/project-a")).toBe("project-a");
    expect(parseProjectIdFromPath("/projects/project-a/chat")).toBe("project-a");
    expect(parseProjectIdFromPath("/projects/project-a/chat/c1")).toBe("project-a");
    expect(parseProjectIdFromPath("/generate")).toBeNull();
    expect(parseProjectIdFromPath("/gallery")).toBeNull();
  });
});
