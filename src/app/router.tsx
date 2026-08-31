import { lazy, Suspense, type ReactElement } from "react";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";

import AppLayout from "../components/layout/AppLayout";
import {
  ROUTES,
  ROUTE_PATHS,
  RouteCategory,
  routeAliases,
} from "./routes/contracts";

const CanvasPage = lazy(() => import("../pages/CanvasPage"));
const FavoritesPage = lazy(() => import("../pages/FavoritesPage"));
const GeneratePage = lazy(() => import("../pages/GeneratePage"));
const GalleryPage = lazy(() => import("../pages/GalleryPage"));
const ProjectChatPage = lazy(() => import("../pages/ProjectChatPage"));
const ProjectHomePage = lazy(() => import("../pages/ProjectHomePage"));
const ProjectsPage = lazy(() => import("../pages/ProjectsPage"));
const PromptExtractPage = lazy(() => import("../pages/PromptExtractPage"));
const SettingsPage = lazy(() => import("../pages/SettingsPage"));
const TrashPage = lazy(() => import("../pages/TrashPage"));

type RoutableCategory = Exclude<RouteCategory, "default">;

function withRouteFallback(page: ReactElement) {
  return (
    <Suspense
      fallback={
        <div
          className="flex h-full items-center justify-center"
          aria-busy="true"
          aria-label="Loading page"
        >
          <div className="h-5 w-5 animate-spin rounded-full border-2 border-primary/20 border-t-primary" />
        </div>
      }
    >
      {page}
    </Suspense>
  );
}

const routeNodeMap: Record<RoutableCategory, ReactElement> = {
  generate: withRouteFallback(<GeneratePage />),
  extract: withRouteFallback(<PromptExtractPage />),
  canvas: withRouteFallback(<CanvasPage />),
  projects: withRouteFallback(<ProjectsPage />),
  chat: withRouteFallback(<ProjectChatPage />),
  projectHome: withRouteFallback(<ProjectHomePage />),
  gallery: withRouteFallback(<GalleryPage />),
  trash: withRouteFallback(<TrashPage />),
  favorites: withRouteFallback(<FavoritesPage />),
  settings: withRouteFallback(<SettingsPage />),
};

const routeDefinitions: Array<{
  kind: RoutableCategory;
  path: string;
  key: string;
}> = [
  { kind: "generate", path: ROUTES.generate, key: "generate" },
  { kind: "extract", path: ROUTES.extract, key: "extract" },
  { kind: "canvas", path: ROUTES.canvas, key: "canvas" },
  { kind: "projects", path: ROUTES.projects, key: "projects" },
  { kind: "projectHome", path: ROUTE_PATHS.projectHome, key: "project-home" },
  {
    kind: "chat",
    path: ROUTE_PATHS.projectChat,
    key: "project-chat",
  },
  {
    kind: "chat",
    path: ROUTE_PATHS.projectChatConversation,
    key: "project-chat-conversation",
  },
  { kind: "gallery", path: ROUTES.gallery, key: "gallery" },
  { kind: "trash", path: ROUTES.trash, key: "trash" },
  { kind: "favorites", path: ROUTES.favorites, key: "favorites" },
  { kind: "settings", path: ROUTES.settings, key: "settings" },
];

export function ApplicationRouter() {
  const defaultRoute = ROUTES.generate;

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          {routeDefinitions.map(({ kind, path, key }) => (
            <Route key={key} path={path} element={routeNodeMap[kind]} />
          ))}
          <Route path="*" element={<Navigate to={defaultRoute} replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}

export const legacyRouteAliases = routeAliases;
