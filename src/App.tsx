import "./i18n";
import { useEffect } from "react";
import { ApplicationRouter } from "./app/router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
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
      <ApplicationRouter />
    </QueryClientProvider>
  );
}

export default App;
