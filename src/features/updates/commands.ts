export { getVersion as getDesktopVersion } from "@tauri-apps/api/app";
import { Channel, invoke } from "@tauri-apps/api/core";
import { flushCanvasSaves } from "../../lib/canvas/persistence";

export interface UpdateMetadata {
  version: string;
  current_version: string;
  body: string | null;
  date: string | null;
}

export type DownloadEvent =
  | { event: "Started"; data: { contentLength: number | null } }
  | {
      event: "Progress";
      data: { chunkLength: number; totalDownloaded: number };
    }
  | { event: "Finished" };

export async function checkForUpdate(): Promise<UpdateMetadata | null> {
  return invoke("check_update");
}

export async function isUpdateSupported(): Promise<boolean> {
  return invoke("is_update_supported");
}

export async function installUpdate(
  onEvent: (event: DownloadEvent) => void,
): Promise<void> {
  const channel = new Channel<DownloadEvent>(onEvent);
  const token = await invoke<string>("download_update", { onEvent: channel });
  await flushCanvasSaves();
  await invoke("install_update", { token });
}

export async function relaunchApp(): Promise<void> {
  await invoke("relaunch_app");
}
