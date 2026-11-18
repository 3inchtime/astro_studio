import { useCallback, useMemo, useState } from "react";
import { normalizePromptFavorite } from "../../lib/generatePageHelpers";
import {
  usePromptFavoritesQuery,
  useCreatePromptFavoriteMutation,
  useDeletePromptFavoriteMutation,
} from "../../lib/queries/favorites";

export function usePromptFavorites() {
  const { data: promptFavorites = [] } = usePromptFavoritesQuery();
  const [promptFavoriteActionKey, setPromptFavoriteActionKey] = useState<
    string | null
  >(null);
  const promptFavoriteCreate = useCreatePromptFavoriteMutation();
  const promptFavoriteDelete = useDeletePromptFavoriteMutation();
  const promptFavoriteByPrompt = useMemo(() => {
    return new Map(
      promptFavorites.map((favorite) => [
        normalizePromptFavorite(favorite.prompt),
        favorite,
      ]),
    );
  }, [promptFavorites]);

  const handleTogglePromptFavorite = useCallback(
    async (value: string) => {
      const promptText = value.trim();
      if (!promptText) return;

      const normalizedPrompt = normalizePromptFavorite(promptText);
      if (promptFavoriteActionKey === normalizedPrompt) return;

      const existing = promptFavoriteByPrompt.get(normalizedPrompt);
      setPromptFavoriteActionKey(normalizedPrompt);
      try {
        if (existing) {
          await promptFavoriteDelete.mutateAsync(existing.id);
        } else {
          await promptFavoriteCreate.mutateAsync(promptText);
        }
      } finally {
        setPromptFavoriteActionKey(null);
      }
    },
    [
      promptFavoriteActionKey,
      promptFavoriteByPrompt,
      promptFavoriteCreate,
      promptFavoriteDelete,
    ],
  );

  return { promptFavoriteByPrompt, handleTogglePromptFavorite };
}
