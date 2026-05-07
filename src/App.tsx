import "./i18n";
import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import AppLayout from "./components/layout/AppLayout";
import GeneratePage from "./pages/GeneratePage";
import ProjectsPage from "./pages/ProjectsPage";
import ProjectHomePage from "./pages/ProjectHomePage";
import ProjectChatPage from "./pages/ProjectChatPage";
import GalleryPage from "./pages/GalleryPage";
import FavoritesPage from "./pages/FavoritesPage";
import SettingsPage from "./pages/SettingsPage";
import TrashPage from "./pages/TrashPage";
import { getFontSize } from "./lib/api";
import { applyAppFontSize, getStoredAppFontSize } from "./lib/fontSize";

applyAppFontSize(getStoredAppFontSize());

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 0,
    },
  },
});

function App() {
  useEffect(() => {
    getFontSize()
      .then((fontSize) => {
        applyAppFontSize(fontSize);
      })
      .catch(() => {
        applyAppFontSize(getStoredAppFontSize());
      });
  }, []);

  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/generate" element={<GeneratePage />} />
            <Route path="/projects" element={<ProjectsPage />} />
            <Route path="/projects/:projectId/chat/:conversationId?" element={<ProjectChatPage />} />
            <Route path="/projects/:projectId" element={<ProjectHomePage />} />
            <Route path="/gallery" element={<GalleryPage />} />
            <Route path="/trash" element={<TrashPage />} />
            <Route path="/favorites" element={<FavoritesPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="*" element={<Navigate to="/generate" replace />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}

export default App;
