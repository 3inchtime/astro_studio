import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Link, MemoryRouter, Route, Routes } from "react-router-dom";
import { expect, it, vi } from "vitest";
import AppLayout, { useLayoutContext } from "../components/layout/AppLayout";
import GeneratePage from "./GeneratePage";

const api = vi.hoisted(() => ({
  getModelProviderProfiles: vi.fn(),
  generateImage: vi.fn().mockResolvedValue({ generation_id: "new", conversation_id: "original-conversation", images: [] }),
}));
vi.mock("../lib/api", async (original) => ({
  ...await original<typeof import("../lib/api")>(),
  getModelProviderProfiles: api.getModelProviderProfiles,
  generateImage: api.generateImage,
  getImageModel: vi.fn().mockResolvedValue("gpt-image-2"),
  getConversationGenerations: vi.fn().mockResolvedValue([]),
  getPromptFavorites: vi.fn().mockResolvedValue([]),
  isUpdateSupported: vi.fn().mockResolvedValue(false),
  onGenerationComplete: vi.fn().mockResolvedValue(() => {}),
  onGenerationFailed: vi.fn().mockResolvedValue(() => {}),
  onGenerationPartial: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("../components/sidebar/ConversationList", () => ({ default: () => null }));
vi.mock("../components/projects/ProjectsSidebar", () => ({ default: () => null }));
vi.mock("../components/generate/RecoveryNotice", () => ({ default: () => null }));
vi.mock("../components/gallery/ExportStatus", () => ({ default: () => null }));
vi.mock("../components/settings/BackupOperationStatus", () => ({ default: () => null }));
vi.mock("../components/generate/GenerationComposer", () => ({
  default: ({ prompt, onPromptChange, onGenerate }: { prompt: string; onPromptChange: (value: string) => void; onGenerate: () => void }) => <>
    <input aria-label="Draft prompt" value={prompt} onChange={(event) => onPromptChange(event.target.value)} />
    <button onClick={onGenerate}>Submit draft</button>
  </>,
}));
function DraftPage() {
  const { activeConversationId } = useLayoutContext();
  return <><span data-testid="active-conversation">{activeConversationId ?? "none"}</span><GeneratePage /></>;
}

it("restores a provider-settings draft into its original real-layout conversation without replay", async () => {
  sessionStorage.clear();
  api.getModelProviderProfiles.mockResolvedValue({ active_provider_id: "profile", profiles: [] });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}>
    <MemoryRouter initialEntries={[{ pathname: "/generate", state: { activateConversationId: "original-conversation" } }]}>
      <Routes><Route element={<AppLayout />}>
        <Route path="/generate" element={<DraftPage />} />
        <Route path="/settings" element={<Link to="/generate">Return to draft</Link>} />
      </Route></Routes>
    </MemoryRouter>
  </QueryClientProvider>);
  await waitFor(() => expect(screen.getByTestId("active-conversation")).toHaveTextContent("original-conversation"));
  fireEvent.change(screen.getByLabelText("Draft prompt"), { target: { value: "Continue the original conversation" } });
  fireEvent.click(await screen.findByRole("button", { name: /configure provider/i }));
  api.getModelProviderProfiles.mockResolvedValue({ active_provider_id: "profile", profiles: [{ id: "profile", name: "Configured", api_key: "", api_key_configured: true, api_key_masked: "••••ture", endpoint_settings: { mode: "base_url", base_url: "https://example.test/v1", generation_url: "", edit_url: "" } }] });
  fireEvent.click(await screen.findByRole("link", { name: "Return to draft" }));
  await waitFor(() => expect(screen.getByTestId("active-conversation")).toHaveTextContent("original-conversation"));
  expect(screen.getByLabelText("Draft prompt")).toHaveValue("Continue the original conversation");
  expect(api.generateImage).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Submit draft" }));
  await waitFor(() => expect(api.generateImage).toHaveBeenCalledWith(expect.objectContaining({ conversationId: "original-conversation", prompt: "Continue the original conversation" })));
});
