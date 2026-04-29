export const DEFAULT_PROMPT_FOLDER_ID = "default";
export const DEFAULT_PROMPT_FOLDER_NAME = "默认收藏夹";

interface PromptFolderLike {
  id: string;
  name: string;
}

export function getPromptFolderDisplayName(folder: PromptFolderLike): string {
  if (folder.id === DEFAULT_PROMPT_FOLDER_ID) {
    return DEFAULT_PROMPT_FOLDER_NAME;
  }

  return folder.name;
}
