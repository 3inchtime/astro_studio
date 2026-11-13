import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import RecoveryNotice from "./RecoveryNotice";
import {
  getGenerationRecoverySummary,
  recoverGeneration,
} from "../../lib/recoveryApi";
vi.mock("../../lib/recoveryApi", () => ({
  getGenerationRecoverySummary: vi.fn(),
  recoverGeneration: vi.fn(),
}));
const empty = {
  recovered: 0,
  failed: 0,
  interrupted: 0,
  pending: 0,
  cancelled: 0,
  entries: [],
};
function Location() {
  const location = useLocation();
  return (
    <output data-testid="location">{JSON.stringify(location.state)}</output>
  );
}
function renderNotice() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <RecoveryNotice />
        <Location />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
describe("RecoveryNotice", () => {
  beforeEach(() => vi.clearAllMocks());
  it("does not show historical error counts without actionable entries", async () => {
    vi.mocked(getGenerationRecoverySummary).mockResolvedValue({
      ...empty,
      failed: 100,
    });
    const view = renderNotice();
    await waitFor(() =>
      expect(getGenerationRecoverySummary).toHaveBeenCalled(),
    );
    expect(view.queryByRole("region")).not.toBeInTheDocument();
  });
  it("retries only local saving and disables duplicate submissions", async () => {
    vi.mocked(getGenerationRecoverySummary).mockResolvedValue({
      ...empty,
      entries: [
        {
          generation_id: "saved",
          conversation_id: "c",
          prompt: "A nebula",
          status: "failed",
          can_recover: true,
          result_unknown: false,
        },
      ],
    });
    vi.mocked(recoverGeneration).mockReturnValue(new Promise(() => {}));
    renderNotice();
    fireEvent.click(
      await screen.findByRole("button", { name: /Review unfinished results/ }),
    );
    expect(screen.getByText(/No new generation request/)).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "Continue saving" });
    fireEvent.click(button);
    fireEvent.click(button);
    await waitFor(() => expect(recoverGeneration).toHaveBeenCalledTimes(1));
    expect(recoverGeneration).toHaveBeenCalledWith("saved");
    expect(screen.getByRole("button", { name: "Saving…" })).toBeDisabled();
  });
  it("explains unknown charges and loads a recipe for review without generating", async () => {
    vi.mocked(getGenerationRecoverySummary).mockResolvedValue({
      ...empty,
      entries: [
        {
          generation_id: "unknown",
          conversation_id: "c",
          prompt: "The sea",
          status: "interrupted",
          can_recover: false,
          result_unknown: true,
        },
      ],
    });
    renderNotice();
    fireEvent.click(
      await screen.findByRole("button", { name: /Review unfinished results/ }),
    );
    expect(screen.getByText(/provider may have charged/)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Continue saving" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "Review recipe" }));
    expect(screen.getByTestId("location")).toHaveTextContent(
      '"recipeGenerationId":"unknown"',
    );
    expect(screen.getByTestId("location")).toHaveTextContent(
      '"recipeMode":"edit"',
    );
    expect(recoverGeneration).not.toHaveBeenCalled();
  });
  it("reports startup recovery even when no unfinished entries remain", async () => {
    vi.mocked(getGenerationRecoverySummary).mockResolvedValue({
      ...empty,
      recovered: 2,
    });
    renderNotice();
    expect(
      await screen.findByText("Saved results recovered at startup: 2."),
    ).toHaveAttribute("role", "status");
  });

  it("distinguishes work interrupted before any provider request", async () => {
    vi.mocked(getGenerationRecoverySummary).mockResolvedValue({
      ...empty,
      entries: [
        {
          generation_id: "unsent",
          conversation_id: "c",
          prompt: "Cloud study",
          status: "interrupted",
          can_recover: false,
          result_unknown: false,
        },
      ],
    });
    renderNotice();
    fireEvent.click(
      await screen.findByRole("button", { name: /Review unfinished results/ }),
    );
    expect(
      screen.getByText(
        "Interrupted before sending. Review the recipe to start a new generation.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/provider may have charged/),
    ).not.toBeInTheDocument();
  });
});
