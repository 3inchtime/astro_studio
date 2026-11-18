import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { cancelGenerationJob } from "../../lib/api";
import CancelGenerationButton from "./CancelGenerationButton";

vi.mock("../../lib/api", () => ({ cancelGenerationJob: vi.fn() }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

it("retries a failed cancellation against the same persisted job and suppresses repeat clicks", async () => {
  vi.mocked(cancelGenerationJob)
    .mockRejectedValueOnce(new Error("temporarily unavailable"))
    .mockImplementationOnce(() => new Promise(() => {}));
  render(<CancelGenerationButton jobId="job-2" />);
  const button = screen.getByRole("button");
  fireEvent.click(button);
  expect(await screen.findByRole("alert")).toHaveTextContent("generate.cancelTaskFailed");
  fireEvent.click(button);
  fireEvent.click(button);
  await waitFor(() => expect(button).toBeDisabled());
  expect(cancelGenerationJob).toHaveBeenCalledTimes(2);
  expect(vi.mocked(cancelGenerationJob).mock.calls).toEqual([["job-2"], ["job-2"]]);
});
