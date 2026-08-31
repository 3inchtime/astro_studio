export type RouteCategory =
  | "generate"
  | "extract"
  | "canvas"
  | "projects"
  | "projectHome"
  | "chat"
  | "gallery"
  | "trash"
  | "favorites"
  | "settings"
  | "default";

export type CanonicalRouteKey = RouteCategory;

export type SidebarLayout = "hidden" | "projects" | "conversations";

export type RouteLayout = {
  kind: RouteCategory;
  sidebar: {
    type: SidebarLayout;
    useProjectFromPath: boolean;
  };
  clearProjectOnEnter: boolean;
};

export type LegacyAliasPolicy = "replace" | "query-preserve";

export type LegacyRouteAlias = {
  from: string;
  to: string;
  policy: LegacyAliasPolicy;
};

export const canonicalRouteNames: Record<RouteCategory, string> = {
  generate: "generate",
  extract: "extract",
  canvas: "canvas",
  projects: "projects",
  projectHome: "projects:projectId",
  chat: "projects:projectId:conversationId",
  gallery: "library",
  trash: "library:trash",
  favorites: "library:favorites",
  settings: "settings",
  default: "home",
};

const encodeRouteSegment = (segment: string) => encodeURIComponent(segment);
const decodeRouteSegment = (segment: string) => {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
};

export const ROUTES = {
  home: "/generate",
  generate: "/generate",
  extract: "/extract",
  canvas: "/canvas",
  projects: "/projects",
  gallery: "/gallery",
  trash: "/trash",
  favorites: "/favorites",
  settings: "/settings",
  projectHome: (projectId: string) =>
    `/projects/${encodeRouteSegment(projectId)}`,
  projectChat: (projectId: string, conversationId?: string | null) =>
    conversationId
      ? `/projects/${encodeRouteSegment(projectId)}/chat/${encodeRouteSegment(conversationId)}`
      : `/projects/${encodeRouteSegment(projectId)}/chat`,
} as const;

export const routeAliases: LegacyRouteAlias[] = [
  { from: ROUTES.projects, to: ROUTES.projects, policy: "replace" },
  { from: ROUTES.generate, to: ROUTES.generate, policy: "replace" },
  { from: ROUTES.canvas, to: ROUTES.canvas, policy: "replace" },
  { from: ROUTES.gallery, to: ROUTES.gallery, policy: "replace" },
  { from: ROUTES.favorites, to: ROUTES.favorites, policy: "replace" },
  { from: ROUTES.trash, to: ROUTES.trash, policy: "replace" },
  { from: ROUTES.extract, to: ROUTES.extract, policy: "replace" },
  { from: ROUTES.settings, to: ROUTES.settings, policy: "replace" },
];

export const ROUTE_PATHS = {
  projectHome: "/projects/:projectId",
  projectChat: "/projects/:projectId/chat",
  projectChatConversation: "/projects/:projectId/chat/:conversationId",
} as const;

export const MAIN_NAV_ROUTES = [
  { key: "generate", to: ROUTES.generate, labelKey: "nav.generate" },
  { key: "extract", to: ROUTES.extract, labelKey: "nav.extract" },
  { key: "canvas", to: ROUTES.canvas, labelKey: "nav.canvas" },
  { key: "projects", to: ROUTES.projects, labelKey: "nav.projects" },
  { key: "gallery", to: ROUTES.gallery, labelKey: "nav.gallery" },
  { key: "favorites", to: ROUTES.favorites, labelKey: "nav.favorites" },
] as const;

export const HIDE_SIDEBAR_PATHS = [
  ROUTES.settings,
  ROUTES.extract,
  ROUTES.canvas,
  ROUTES.gallery,
  ROUTES.favorites,
] as const;

const HIDE_SIDEBAR_KINDS = new Set([
  "settings",
  "extract",
  "canvas",
  "gallery",
  "favorites",
]);

export const ROUTE_LAYOUTS: Record<RouteCategory, RouteLayout> = {
  generate: {
    kind: "generate",
    sidebar: { type: "conversations", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
  extract: {
    kind: "extract",
    sidebar: { type: "hidden", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
  canvas: {
    kind: "canvas",
    sidebar: { type: "hidden", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
  projects: {
    kind: "projects",
    sidebar: { type: "projects", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
  projectHome: {
    kind: "projectHome",
    sidebar: { type: "conversations", useProjectFromPath: true },
    clearProjectOnEnter: false,
  },
  chat: {
    kind: "chat",
    sidebar: { type: "conversations", useProjectFromPath: true },
    clearProjectOnEnter: false,
  },
  gallery: {
    kind: "gallery",
    sidebar: { type: "hidden", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
  trash: {
    kind: "trash",
    sidebar: { type: "conversations", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
  favorites: {
    kind: "favorites",
    sidebar: { type: "hidden", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
  settings: {
    kind: "settings",
    sidebar: { type: "hidden", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
  default: {
    kind: "default",
    sidebar: { type: "conversations", useProjectFromPath: false },
    clearProjectOnEnter: true,
  },
};

export type RouteContext =
  | { kind: "generate"; pathname: string; projectId: null; conversationId: null }
  | {
      kind:
        | "projects"
        | "gallery"
        | "trash"
        | "favorites"
        | "extract"
        | "canvas";
      pathname: string;
      projectId: null;
      conversationId: null;
    }
  | {
      kind: "projectHome";
      pathname: string;
      projectId: string;
      conversationId: null;
    }
  | {
      kind: "chat";
      pathname: string;
      projectId: string;
      conversationId: string | null;
    }
  | { kind: "settings"; pathname: string; projectId: null; conversationId: null }
  | { kind: "unknown"; pathname: string; projectId: null; conversationId: null };

export const ROUTE_PATTERNS = {
  projectHome: /^\/projects\/([^/]+)$/,
  projectChat: /^\/projects\/([^/]+)\/chat(?:\/([^/]+))?$/,
} as const;

function normalizePath(pathname: string): string {
  if (!pathname) {
    return "/";
  }

  const stripped = pathname.split("?")[0].split("#")[0];
  const collapsed = stripped.replace(/\/+/g, "/");

  if (collapsed.length > 1 && collapsed.endsWith("/")) {
    return collapsed.slice(0, -1);
  }

  return collapsed;
}

export function getRouteContext(pathname: string): RouteContext {
  const normalizedPathname = normalizePath(pathname);

  if (normalizedPathname === ROUTES.generate) {
    return {
      kind: "generate",
      pathname: normalizedPathname,
      projectId: null,
      conversationId: null,
    };
  }

  if (normalizedPathname === ROUTES.projects) {
    return {
      kind: "projects",
      pathname: normalizedPathname,
      projectId: null,
      conversationId: null,
    };
  }

  const projectHomeMatch = ROUTE_PATTERNS.projectHome.exec(normalizedPathname);
  if (projectHomeMatch) {
    return {
      kind: "projectHome",
      pathname: normalizedPathname,
      projectId: decodeRouteSegment(projectHomeMatch[1] ?? ""),
      conversationId: null,
    };
  }

  const projectChatMatch = ROUTE_PATTERNS.projectChat.exec(normalizedPathname);
  if (projectChatMatch) {
    return {
      kind: "chat",
      pathname: normalizedPathname,
      projectId: decodeRouteSegment(projectChatMatch[1] ?? ""),
      conversationId: projectChatMatch[2]
        ? decodeRouteSegment(projectChatMatch[2])
        : null,
    };
  }

  if (normalizedPathname === ROUTES.gallery) {
    return {
      kind: "gallery",
      pathname: normalizedPathname,
      projectId: null,
      conversationId: null,
    };
  }

  if (normalizedPathname === ROUTES.favorites) {
    return {
      kind: "favorites",
      pathname: normalizedPathname,
      projectId: null,
      conversationId: null,
    };
  }

  if (normalizedPathname === ROUTES.extract) {
    return {
      kind: "extract",
      pathname: normalizedPathname,
      projectId: null,
      conversationId: null,
    };
  }

  if (normalizedPathname === ROUTES.canvas) {
    return {
      kind: "canvas",
      pathname: normalizedPathname,
      projectId: null,
      conversationId: null,
    };
  }

  if (normalizedPathname === ROUTES.settings) {
    return {
      kind: "settings",
      pathname: normalizedPathname,
      projectId: null,
      conversationId: null,
    };
  }

  if (normalizedPathname === ROUTES.trash) {
    return {
      kind: "trash",
      pathname: normalizedPathname,
      projectId: null,
      conversationId: null,
    };
  }

  return {
    kind: "unknown",
    pathname: normalizedPathname,
    projectId: null,
    conversationId: null,
  };
}

export function isGenerateRoute(pathname: string) {
  return getRouteContext(pathname).kind === "generate";
}

export function isProjectsRoute(pathname: string) {
  return getRouteContext(pathname).kind === "projects";
}

export function isProjectHomeRoute(pathname: string) {
  return getRouteContext(pathname).kind === "projectHome";
}

export function isProjectChatRoute(pathname: string) {
  return getRouteContext(pathname).kind === "chat";
}

export function isHideSidebarRoute(pathname: string) {
  return shouldHideSidebarForContext(getRouteContext(pathname));
}

export function shouldHideSidebarForContext(context: RouteContext) {
  return HIDE_SIDEBAR_KINDS.has(context.kind);
}

export function getRouteLayout(kind: RouteContext["kind"]) {
  if (kind === "unknown") {
    return ROUTE_LAYOUTS.default;
  }

  return ROUTE_LAYOUTS[kind];
}

export function parseProjectIdFromPath(pathname: string) {
  const context = getRouteContext(pathname);
  return context.kind === "projectHome" || context.kind === "chat"
    ? context.projectId
    : null;
}
