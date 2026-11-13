import { MemoryRouter } from "react-router-dom";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProviderDiagnostics } from "./ProviderDiagnostics";
import {
  diagnoseImageProvider,
  getProviderSmokePlan,
  runProviderSmokeTest,
} from "../../lib/diagnosticsApi";
vi.mock("../../lib/diagnosticsApi", () => ({
  diagnoseImageProvider: vi.fn(),
  getProviderSmokePlan: vi.fn(),
  runProviderSmokeTest: vi.fn(),
}));
const profile = {
  id: "p",
  name: "Saved provider",
  api_key: "key",
  endpoint_settings: {
    mode: "base_url" as const,
    base_url: "https://example.test/v1",
    generation_url: "",
    edit_url: "",
  },
};
const result = {
  local: { status: "passed", code: "valid" },
  transport: { status: "passed", code: "reachable" },
  authentication: { status: "passed", code: "accepted" },
  model: { status: "passed", code: "available" },
  generation: { status: "unverified", code: "not_generated" },
};
describe("provider diagnostics", () => {
  beforeEach(() => vi.clearAllMocks());
  it("links the saved test conversation after an unknown provider outcome", async () => {
    vi.mocked(getProviderSmokePlan).mockResolvedValue({
      model: "gpt-image-2", provider_id: "p", provider_name: "Saved provider",
      endpoint_host: "example.test", fingerprint: "f", prompt: "A circle",
      image_count: 1, quality: "low", size: "1024x1024", resolution: null,
      aspect_ratio: null, output_format: "png",
    });
    vi.mocked(runProviderSmokeTest).mockRejectedValue({
      code: "network_error", conversation_id: "failed-test-conversation",
    });
    render(<ProviderDiagnostics model="gpt-image-2" profile={profile} dirty={false} />, { wrapper: MemoryRouter });
    fireEvent.click(screen.getByRole("button", { name: "Test one image…" }));
    await screen.findByRole("dialog");
    fireEvent.click(screen.getByRole("button", { name: "Generate one test image" }));
    expect(await screen.findByRole("link", { name: "Open test conversation" })).toHaveAttribute("href", "/projects/default/chat/failed-test-conversation");
    expect(screen.getByText(/charge may still apply/)).toBeInTheDocument();
    expect(runProviderSmokeTest).toHaveBeenCalledTimes(1);
  });
  it("requires saved settings and ignores a response after edits", async () => {
    let resolve!: (value: unknown) => void;
    vi.mocked(diagnoseImageProvider).mockReturnValue(
      new Promise((r) => {
        resolve = r;
      }) as never,
    );
    const view = render(
      <ProviderDiagnostics
        model="gpt-image-2"
        profile={profile}
        dirty={false}
      />,
      { wrapper: MemoryRouter },
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Diagnose connection" }),
    );
    view.rerender(
      <ProviderDiagnostics
        model="gpt-image-2"
        profile={{ ...profile, api_key: "changed" }}
        dirty
      />,
    );
    await act(async () => resolve(result));
    expect(
      screen.getByRole("button", { name: "Diagnose connection" }),
    ).toBeDisabled();
    expect(screen.queryByText("Model available")).not.toBeInTheDocument();
    expect(
      screen.getByText("Save your changes before testing this provider."),
    ).toBeInTheDocument();
  });
  it("requires confirmation with exact plan before one potentially paid generation", async () => {
    vi.mocked(getProviderSmokePlan).mockResolvedValue({
      model: "gpt-image-2",
      provider_id: "p",
      provider_name: "Saved provider",
      endpoint_host: "example.test",
      fingerprint: "f",
      prompt: "A small blue circle on a plain white background.",
      image_count: 1,
      quality: "low",
      size: "1024x1024",
      resolution: null,
      aspect_ratio: null,
      output_format: "png",
    });
    vi.mocked(runProviderSmokeTest).mockResolvedValue({
      generation_id: "g",
      conversation_id: "c",
    } as never);
    render(
      <ProviderDiagnostics
        model="gpt-image-2"
        profile={profile}
        dirty={false}
      />,
      { wrapper: MemoryRouter },
    );
    fireEvent.click(screen.getByRole("button", { name: "Test one image…" }));
    await screen.findByRole("dialog");
    expect(screen.getByText(/1024x1024/)).toBeInTheDocument();
    expect(screen.getByText(/may incur a charge/)).toBeInTheDocument();
    expect(runProviderSmokeTest).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("button", { name: "Generate one test image" }),
    );
    await waitFor(() => expect(runProviderSmokeTest).toHaveBeenCalledTimes(1));
    expect(runProviderSmokeTest).toHaveBeenCalledWith(
      "gpt-image-2",
      "p",
      "f",
      expect.any(String),
    );
  });
});
