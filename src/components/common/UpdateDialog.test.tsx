import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import UpdateDialog from "./UpdateDialog";
import type { DownloadEvent } from "../../lib/api";
const native = vi.hoisted(() => ({ installUpdate: vi.fn(), relaunchApp: vi.fn() }));
vi.mock("../../lib/api", () => native);
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
afterEach(cleanup);
it("keeps the dialog blocked until verification, canvas flush and installation complete", async () => {
  let finish!: () => void;
  native.installUpdate.mockImplementation(async (onEvent: (event: DownloadEvent) => void) => {
    onEvent({ event: "Finished" });
    await new Promise<void>((resolve) => { finish = resolve; });
  });
  const close = vi.fn();
  render(<UpdateDialog open update={{version:"1.0.0",current_version:"0.0.34",body:null,date:null}} onClose={close} />);
  fireEvent.click(screen.getByRole("button", { name: "update.download" }));
  expect(screen.queryByRole("button", { name: "update.relaunch" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "update.close" })).not.toBeInTheDocument();
  await act(async () => { finish(); });
  expect(screen.getByRole("button", { name: "update.relaunch" })).toBeInTheDocument();
});
