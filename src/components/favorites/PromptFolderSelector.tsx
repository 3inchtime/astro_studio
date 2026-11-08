import {
  addPromptFavoriteToFolders,
  createPromptFolder,
  getPromptFavoriteFolders,
  getPromptFolders,
  removePromptFavoriteFromFolders,
} from "../../lib/api";
import { getPromptFolderDisplayName } from "../../lib/promptFolders";
import { FolderPickerDialog, type FolderDialogService } from "./FolderSelector";

const promptFolders: FolderDialogService = {
  list: getPromptFolders,
  selected: getPromptFavoriteFolders,
  create: createPromptFolder,
  add: addPromptFavoriteToFolders,
  remove: removePromptFavoriteFromFolders,
};

export default function PromptFolderSelector({ favoriteId, onClose }: {
  favoriteId: string;
  onClose: () => void;
}) {
  return <FolderPickerDialog itemId={favoriteId} titleKey="favorites.addPromptToFolder" service={promptFolders} displayName={getPromptFolderDisplayName} onClose={onClose} />;
}
