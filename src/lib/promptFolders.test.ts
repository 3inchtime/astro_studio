import { describe, expect, it } from "vitest";
import {
  DEFAULT_PROMPT_FOLDER_NAME,
  getPromptFolderDisplayName,
} from "./promptFolders";

describe("prompt folder display names", () => {
  it("normalizes the built-in default folder label", () => {
    expect(
      getPromptFolderDisplayName({
        id: "default",
        name: "Default",
      }),
    ).toBe(DEFAULT_PROMPT_FOLDER_NAME);
  });

  it("preserves custom prompt folder names", () => {
    expect(
      getPromptFolderDisplayName({
        id: "folder-1",
        name: "Characters",
      }),
    ).toBe("Characters");
  });
});
