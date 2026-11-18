import "./i18n";
import CanvasCloseGuard from "./components/canvas/CanvasCloseGuard";
import { useEffect } from "react";
import { ApplicationRouter } from "./app/router";
import { queryClient } from "./lib/queryClient";
import { QueryClientProvider } from "@tanstack/react-query";
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
    <QueryClientProvider client={queryClient}>
      <ApplicationRouter />
      <CanvasCloseGuard />
    </QueryClientProvider>
  );
}

export default App;
