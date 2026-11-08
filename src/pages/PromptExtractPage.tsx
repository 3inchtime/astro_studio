import { useCallback, useEffect, useMemo, useState } from "react";
import { Copy, Heart, ImagePlus, Sparkles } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { createConversation, pickSourceImages, toAssetUrl } from "../lib/api";
import { normalizePromptFavorite } from "../lib/generatePageHelpers";
import { normalizeLanguage } from "../lib/languages";
import { formatLocalDateTime } from "../lib/utils";
import {
  useCreatePromptFavoriteMutation,
  useDeletePromptFavoriteMutation,
  usePromptFavoritesQuery,
} from "../lib/queries/favorites";
import {
  useExtractPromptFromImageMutation,
  useLlmConfigsQuery,
  usePromptExtractionsQuery,
} from "../lib/queries/llm";
import { ROUTES } from "../app/routes/contracts";

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
  const [selectedConfigId, setSelectedConfigId] = useState("");
  const [copyError, setCopyError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [favoriteError, setFavoriteError] = useState<string | null>(null);

  const multimodalConfigs = useMemo(
    () =>
      llmConfigs.filter(
        (config) => config.enabled && config.capability === "multimodal",
      ),
    [llmConfigs],
  );
  useEffect(() => {
    if (!multimodalConfigs.some((config) => config.id === selectedConfigId)) {
      setSelectedConfigId(multimodalConfigs[0]?.id ?? "");
    }
  }, [multimodalConfigs, selectedConfigId]);
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
    setActionError(null);
    try {
      const paths = await pickSourceImages();
      if (paths.length > 0) setImagePath(paths[0]);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    }
  }, []);

  const handleExtract = useCallback(async () => {
    if (!imagePath || !selectedConfigId) return;

    setActionError(null);
    setCopied(false);
    setCopyError(null);
    try {
      const result = await extractMutation.mutateAsync({
        imagePath,
        configId: selectedConfigId,
        language,
      });
      setPrompt(result.prompt);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    }
  }, [extractMutation, imagePath, language, selectedConfigId]);

  const handleCopy = useCallback(async () => {
    if (!prompt) return;

    setCopied(false);
    setCopyError(null);
    try {
      await navigator.clipboard.writeText(prompt);
      setCopied(true);
    } catch {
      setCopyError(
        t("studio.copyFailed", { defaultValue: "Could not copy. Try again." }),
      );
    }
  }, [prompt, t]);

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

    setActionError(null);
    try {
      const conversation = await createConversation();
      navigate(ROUTES.generate, {
        state: {
          pendingPrompt: prompt,
          activateConversationId: conversation.id,
        },
      });
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    }
  }, [navigate, prompt]);

  const canExtract =
    !!imagePath && !!selectedConfigId && !extractMutation.isPending;

  const handleSelectHistory = useCallback(
    (historyItem: { image_path: string; prompt: string }) => {
      setImagePath(historyItem.image_path);
      setPrompt(historyItem.prompt);
      setCopied(false);
      setFavoriteError(null);
      setCopyError(null);
      setActionError(null);
    },
    [],
  );

  return (
    <div className="flex h-full flex-col overflow-auto bg-background px-6 py-6">
      <div className="mx-auto flex w-full max-w-6xl flex-1 flex-col gap-6">
        <header>
          <h1 className="text-[20px] font-semibold tracking-tight text-foreground">
            {t("extract.title")}
          </h1>
          <p className="mt-2 max-w-xl text-[14px] leading-relaxed text-muted">
            {t("extract.subtitle")}
          </p>
        </header>

        <div className="grid gap-5 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
          <section className="studio-panel flex flex-col gap-4 rounded-xl p-5">
            <div>
              <h2 className="text-[14px] font-semibold text-foreground">
                {t("extract.uploadTitle")}
              </h2>
              <p className="mt-1 text-[12px] text-muted">
                {t("extract.uploadHint")}
              </p>
            </div>

            <button
              type="button"
              onClick={handlePickImage}
              className="focus-ring flex min-h-[280px] cursor-pointer flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border bg-background px-6 py-8 text-center transition-colors hover:border-primary"
            >
              {imagePath ? (
                <img
                  src={toAssetUrl(imagePath)}
                  alt=""
                  className="max-h-[280px] w-full rounded-lg object-contain"
                />
              ) : (
                <>
                  <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-border bg-surface text-muted">
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

            <label className="flex flex-col gap-1.5 text-[12px] font-medium text-muted">
              {t("studio.visionModel", { defaultValue: "Vision model" })}
              <select
                value={selectedConfigId}
                onChange={(event) => setSelectedConfigId(event.target.value)}
                disabled={
                  multimodalConfigs.length === 0 || extractMutation.isPending
                }
                className="studio-input focus-ring select-control h-9 w-full rounded-lg px-3 pr-8 text-[14px] text-foreground"
              >
                {multimodalConfigs.length === 0 && (
                  <option value="">
                    {t("studio.noVisionModel", {
                      defaultValue: "No vision model available",
                    })}
                  </option>
                )}
                {multimodalConfigs.map((config) => (
                  <option key={config.id} value={config.id}>
                    {config.name || config.model}
                  </option>
                ))}
              </select>
            </label>

            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate text-[14px] font-medium text-foreground">
                  {imagePath || t("extract.noImageSelected")}
                </p>
              </div>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={handlePickImage}
                  className="studio-control focus-ring rounded-lg px-4 py-2 text-[12px] font-medium hover:studio-control-hover"
                >
                  {imagePath
                    ? t("extract.changeImage")
                    : t("extract.selectImage")}
                </button>
                <button
                  type="button"
                  onClick={handleExtract}
                  disabled={!canExtract}
                  className="studio-control-primary focus-ring inline-flex items-center gap-2 rounded-lg px-4 py-2 text-[14px] font-semibold disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <Sparkles size={13} />
                  {extractMutation.isPending
                    ? t("extract.extracting")
                    : t("extract.extractPrompt")}
                </button>
              </div>
            </div>

            {actionError && (
              <p
                role="alert"
                className="rounded-lg border border-error/20 bg-error/5 px-3 py-2 text-[12px] text-error"
              >
                {actionError}
              </p>
            )}
            {!selectedConfigId && (
              <div className="rounded-xl border border-warning/20 bg-warning/8 px-4 py-3 text-[12px] text-warning">
                {t("extract.noMultimodalConfig")}
              </div>
            )}
          </section>

          <section className="studio-panel flex min-h-0 flex-col gap-4 rounded-xl p-5">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h2 className="text-[14px] font-semibold text-foreground">
                  {t("extract.resultTitle")}
                </h2>
                <p className="mt-1 text-[12px] text-muted">
                  {t("extract.resultHint")}
                </p>
              </div>
            </div>

            <textarea
              value={prompt}
              aria-label={t("extract.resultTitle")}
              onChange={(event) => {
                setPrompt(event.target.value);
                setCopied(false);
                setCopyError(null);
              }}
              placeholder={t("extract.resultPlaceholder")}
              spellCheck={false}
              className="studio-input focus-ring min-h-[300px] flex-1 w-full resize-y rounded-lg px-4 py-3 text-[14px] leading-relaxed placeholder:text-muted focus:border-primary"
            />

            {copyError && (
              <p role="alert" className="text-[12px] text-error">
                {copyError}
              </p>
            )}
            {favoriteError && (
              <div
                role="alert"
                className="rounded-xl border border-error/20 bg-error/8 px-4 py-3 text-[12px] text-error"
              >
                {favoriteError}
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={handleCopy}
                disabled={!prompt}
                className="studio-control focus-ring inline-flex items-center gap-2 rounded-lg px-4 py-2 text-[12px] font-medium hover:studio-control-hover disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Copy size={13} />
                {copied ? t("extract.copied") : t("extract.copy")}
              </button>
              <button
                type="button"
                onClick={handleFavorite}
                disabled={!prompt}
                className="studio-control focus-ring inline-flex items-center gap-2 rounded-lg px-4 py-2 text-[12px] font-medium hover:studio-control-hover disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Heart
                  size={13}
                  fill={favoritedPrompt ? "currentColor" : "none"}
                />
                {favoritedPrompt
                  ? t("extract.unfavorite")
                  : t("extract.favorite")}
              </button>
              <button
                type="button"
                onClick={handleUsePrompt}
                disabled={!prompt}
                className="studio-control-primary focus-ring inline-flex items-center gap-2 rounded-lg px-4 py-2 text-[14px] font-semibold disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Sparkles size={13} />
                {t("extract.usePrompt")}
              </button>
            </div>
          </section>
        </div>

        <section className="studio-panel rounded-xl p-5">
          <div className="flex items-center justify-between gap-3">
            <div>
              <h2 className="text-[14px] font-semibold text-foreground">
                {t("extract.historyTitle")}
              </h2>
              <p className="mt-1 text-[12px] text-muted">
                {t("extract.historyHint")}
              </p>
            </div>
          </div>

          {promptExtractions.length === 0 ? (
            <div className="mt-4 rounded-xl border border-dashed border-border-subtle px-4 py-6 text-center text-[12px] text-muted">
              {t("extract.historyEmpty")}
            </div>
          ) : (
            <div className="mt-4 grid gap-3 md:grid-cols-2">
              {promptExtractions.map((historyItem) => {
                const isActive =
                  historyItem.image_path === imagePath &&
                  historyItem.prompt === prompt;

                return (
                  <button
                    key={historyItem.id}
                    type="button"
                    aria-label={historyItem.prompt}
                    onClick={() => handleSelectHistory(historyItem)}
                    className={`focus-ring flex cursor-pointer items-start gap-3 rounded-xl border px-3 py-3 text-left transition-colors ${
                      isActive
                        ? "border-primary/35 bg-primary/6"
                        : "border-border-subtle bg-surface hover:bg-subtle"
                    }`}
                  >
                    <img
                      src={toAssetUrl(historyItem.image_path)}
                      alt=""
                      className="h-14 w-14 shrink-0 rounded-lg object-cover"
                    />
                    <div className="min-w-0 flex-1">
                      <p className="text-[12px] text-muted">
                        {formatLocalDateTime(historyItem.updated_at)}
                      </p>
                      <p className="mt-1 line-clamp-2 text-[14px] font-medium leading-relaxed text-foreground">
                        {historyItem.prompt}
                      </p>
                      <p className="mt-1 truncate text-[12px] text-muted">
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
