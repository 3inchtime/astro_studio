import { useCallback, useEffect, useMemo, useState } from "react";
import { Copy, Heart, ImagePlus, Sparkles } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { createConversation, pickSourceImages, toAssetUrl } from "../lib/api";
import { normalizePromptFavorite } from "../lib/generatePageHelpers";
import { normalizeLanguage } from "../lib/languages";
import { formatLocalDateTime } from "../lib/utils";
import { useCreatePromptFavoriteMutation, useDeletePromptFavoriteMutation, usePromptFavoritesQuery } from "../lib/queries/favorites";
import { useExtractPromptFromImageMutation, useLlmConfigsQuery, usePromptExtractionsQuery } from "../lib/queries/llm";

export default function PromptExtractPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const { data: llmConfigs = [] } = useLlmConfigsQuery();
  const { data: promptFavorites = [] } = usePromptFavoritesQuery();
  const { data: promptExtractions = [] } = usePromptExtractionsQuery(20);
  const extractMutation = useExtractPromptFromImageMutation();
  const createFavoriteMutation = useCreatePromptFavoriteMutation();
  const deleteFavoriteMutation = useDeletePromptFavoriteMutation();
  const [imagePath, setImagePath] = useState("");
  const [prompt, setPrompt] = useState("");
  const [copied, setCopied] = useState(false);
  const [favoriteError, setFavoriteError] = useState<string | null>(null);

  const multimodalConfigs = useMemo(
    () => llmConfigs.filter((config) => config.enabled && config.capability === "multimodal"),
    [llmConfigs],
  );
  const selectedConfigId = multimodalConfigs[0]?.id ?? "";
  const language = normalizeLanguage(i18n.resolvedLanguage ?? i18n.language);
  const normalizedPrompt = normalizePromptFavorite(prompt);
  const favoritedPrompt = promptFavorites.find(
    (favorite) => normalizePromptFavorite(favorite.prompt) === normalizedPrompt,
  );

  useEffect(() => {
    if (!copied) return;

    const timer = window.setTimeout(() => setCopied(false), 1200);
    return () => window.clearTimeout(timer);
  }, [copied]);

  const handlePickImage = useCallback(async () => {
    const paths = await pickSourceImages();
    if (paths.length === 0) return;

    setImagePath(paths[0]);
  }, []);

  const handleExtract = useCallback(async () => {
    if (!imagePath || !selectedConfigId) return;

    const result = await extractMutation.mutateAsync({
      imagePath,
      configId: selectedConfigId,
      language,
    });
    setPrompt(result.prompt);
  }, [extractMutation, imagePath, language, selectedConfigId]);

  const handleCopy = useCallback(async () => {
    if (!prompt) return;

    await navigator.clipboard.writeText(prompt).catch(() => {});
    setCopied(true);
  }, [prompt]);

  const handleFavorite = useCallback(async () => {
    if (!prompt) return;

    setFavoriteError(null);
    try {
      if (favoritedPrompt) {
        await deleteFavoriteMutation.mutateAsync(favoritedPrompt.id);
        return;
      }

      await createFavoriteMutation.mutateAsync(prompt);
    } catch (error) {
      setFavoriteError(error instanceof Error ? error.message : String(error));
    }
  }, [createFavoriteMutation, deleteFavoriteMutation, favoritedPrompt, prompt]);

  const handleUsePrompt = useCallback(async () => {
    if (!prompt) return;

    const conversation = await createConversation();
    navigate("/generate", {
      state: {
        pendingPrompt: prompt,
        activateConversationId: conversation.id,
      },
    });
  }, [navigate, prompt]);

  const canExtract = !!imagePath && !!selectedConfigId && !extractMutation.isPending;

  const handleSelectHistory = useCallback((historyItem: {
    image_path: string;
    prompt: string;
  }) => {
    setImagePath(historyItem.image_path);
    setPrompt(historyItem.prompt);
    setCopied(false);
    setFavoriteError(null);
  }, []);

  return (
    <div className="flex h-full flex-col overflow-auto px-6 py-6">
      <div className="mx-auto flex w-full max-w-6xl flex-1 flex-col gap-6">
        <header className="rounded-[18px] border border-border-subtle bg-glass px-6 py-5 shadow-card">
          <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-primary/80">
            {t("extract.title")}
          </p>
          <h1 className="mt-1 text-[24px] font-semibold tracking-tight text-foreground">
            {t("extract.title")}
          </h1>
          <p className="mt-2 max-w-xl text-[13px] leading-relaxed text-muted">
            {t("extract.subtitle")}
          </p>
        </header>

        <div className="grid gap-6 lg:grid-cols-[minmax(0,0.88fr)_minmax(0,1.12fr)]">
          <section className="flex flex-col gap-4 rounded-[18px] border border-border-subtle bg-surface p-5 shadow-card">
            <div>
              <h2 className="text-[15px] font-semibold text-foreground">
                {t("extract.uploadTitle")}
              </h2>
              <p className="mt-1 text-[12px] text-muted">
                {t("extract.uploadHint")}
              </p>
            </div>

            <button
              type="button"
              onClick={handlePickImage}
              className="flex min-h-[320px] flex-col items-center justify-center gap-3 rounded-[16px] border border-dashed border-primary/20 bg-gradient-to-br from-primary/6 via-surface to-accent/6 px-6 py-8 text-center transition-colors hover:border-primary/35"
            >
              {imagePath ? (
                <img
                  src={toAssetUrl(imagePath)}
                  alt=""
                  className="max-h-[280px] w-full rounded-[12px] object-cover shadow-card"
                />
              ) : (
                <>
                  <div className="flex h-14 w-14 items-center justify-center rounded-[16px] bg-primary/10 text-primary">
                    <ImagePlus size={24} />
                  </div>
                  <div className="space-y-1">
                    <strong className="block text-[14px] font-semibold text-foreground">
                      {t("extract.selectImage")}
                    </strong>
                    <p className="text-[12px] leading-relaxed text-muted">
                      {t("extract.uploadHint")}
                    </p>
                  </div>
                </>
              )}
            </button>

            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate text-[13px] font-medium text-foreground">
                  {imagePath || t("extract.noImageSelected")}
                </p>
              </div>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={handlePickImage}
                  className="rounded-[10px] border border-border-subtle bg-surface px-4 py-2 text-[12px] font-medium text-foreground/80 transition-colors hover:bg-subtle"
                >
                  {imagePath ? t("extract.changeImage") : t("extract.selectImage")}
                </button>
                <button
                  type="button"
                  onClick={handleExtract}
                  disabled={!canExtract}
                  className="inline-flex items-center gap-2 rounded-[10px] gradient-primary px-4 py-2 text-[12px] font-semibold text-white shadow-button disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <Sparkles size={13} />
                  {extractMutation.isPending
                    ? t("extract.extracting")
                    : t("extract.extractPrompt")}
                </button>
              </div>
            </div>

            {!selectedConfigId && (
              <div className="rounded-[12px] border border-warning/20 bg-warning/8 px-4 py-3 text-[12px] text-warning">
                {t("extract.noMultimodalConfig")}
              </div>
            )}
          </section>

          <section className="flex min-h-0 flex-col gap-4 rounded-[18px] border border-border-subtle bg-surface p-5 shadow-card">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h2 className="text-[15px] font-semibold text-foreground">
                  {t("extract.resultTitle")}
                </h2>
                <p className="mt-1 text-[12px] text-muted">
                  {t("extract.resultHint")}
                </p>
              </div>
            </div>

            <textarea
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              placeholder={t("extract.resultPlaceholder")}
              spellCheck={false}
              className="min-h-[260px] w-full resize-none rounded-[16px] border border-border-subtle bg-subtle/40 px-4 py-4 text-[14px] leading-[1.75] text-foreground outline-none transition-colors placeholder:text-muted/50 focus:border-primary/30 focus:bg-surface"
            />

            {favoriteError && (
              <div className="rounded-[12px] border border-error/20 bg-error/8 px-4 py-3 text-[12px] text-error">
                {favoriteError}
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={handleCopy}
                disabled={!prompt}
                className="inline-flex items-center gap-2 rounded-[10px] border border-border-subtle bg-surface px-4 py-2 text-[12px] font-medium text-foreground/80 transition-colors hover:bg-subtle disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Copy size={13} />
                {copied ? t("extract.copied") : t("extract.copy")}
              </button>
              <button
                type="button"
                onClick={handleFavorite}
                disabled={!prompt}
                className="inline-flex items-center gap-2 rounded-[10px] border border-border-subtle bg-surface px-4 py-2 text-[12px] font-medium text-foreground/80 transition-colors hover:bg-subtle disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Heart size={13} fill={favoritedPrompt ? "currentColor" : "none"} />
                {favoritedPrompt ? t("extract.unfavorite") : t("extract.favorite")}
              </button>
              <button
                type="button"
                onClick={handleUsePrompt}
                disabled={!prompt}
                className="inline-flex items-center gap-2 rounded-[10px] gradient-primary px-4 py-2 text-[12px] font-semibold text-white shadow-button disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Sparkles size={13} />
                {t("extract.usePrompt")}
              </button>
            </div>
          </section>
        </div>

        <section className="rounded-[18px] border border-border-subtle bg-surface p-5 shadow-card">
          <div className="flex items-center justify-between gap-3">
            <div>
              <h2 className="text-[15px] font-semibold text-foreground">
                {t("extract.historyTitle")}
              </h2>
              <p className="mt-1 text-[12px] text-muted">
                {t("extract.historyHint")}
              </p>
            </div>
          </div>

          {promptExtractions.length === 0 ? (
            <div className="mt-4 rounded-[12px] border border-dashed border-border-subtle px-4 py-6 text-center text-[12px] text-muted">
              {t("extract.historyEmpty")}
            </div>
          ) : (
            <div className="mt-4 grid gap-3 md:grid-cols-2">
              {promptExtractions.map((historyItem) => {
                const isActive =
                  historyItem.image_path === imagePath && historyItem.prompt === prompt;

                return (
                  <button
                    key={historyItem.id}
                    type="button"
                    aria-label={historyItem.prompt}
                    onClick={() => handleSelectHistory(historyItem)}
                    className={`flex items-start gap-3 rounded-[14px] border px-3 py-3 text-left transition-colors ${
                      isActive
                        ? "border-primary/35 bg-primary/6"
                        : "border-border-subtle bg-surface hover:bg-subtle"
                    }`}
                  >
                    <img
                      src={toAssetUrl(historyItem.image_path)}
                      alt=""
                      className="h-14 w-14 shrink-0 rounded-[10px] object-cover"
                    />
                    <div className="min-w-0 flex-1">
                      <p className="text-[11px] text-muted">
                        {formatLocalDateTime(historyItem.updated_at)}
                      </p>
                      <p className="mt-1 line-clamp-2 text-[13px] font-medium leading-relaxed text-foreground">
                        {historyItem.prompt}
                      </p>
                      <p className="mt-1 truncate text-[11px] text-muted">
                        {historyItem.image_path}
                      </p>
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
