import "./i18n";
import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import AppLayout from "./components/layout/AppLayout";
import GeneratePage from "./pages/GeneratePage";
import GalleryPage from "./pages/GalleryPage";
import FavoritesPage from "./pages/FavoritesPage";
import SettingsPage from "./pages/SettingsPage";
import TrashPage from "./pages/TrashPage";
import { getFontSize } from "./lib/api";
import { applyAppFontSize, getStoredAppFontSize } from "./lib/fontSize";

applyAppFontSize(getStoredAppFontSize());

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
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="/generate" element={<GeneratePage />} />
          <Route path="/gallery" element={<GalleryPage />} />
          <Route path="/trash" element={<TrashPage />} />
          <Route path="/favorites" element={<FavoritesPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/generate" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}

export default App;
