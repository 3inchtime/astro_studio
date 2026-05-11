import { motion, AnimatePresence } from "framer-motion";
import { ArrowUp, ChevronDown, ImagePlus, Loader2, Sparkles, Wand2, X } from "lucide-react";
import type { RefObject } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toAssetUrl } from "../../lib/api";
import {
  IMAGE_MODEL_CATALOG,
  getImageModelCatalogEntry,
} from "../../lib/modelCatalog";
import { useLlmConfigsQuery, useOptimizePromptMutation } from "../../lib/queries/llm";
import type {
  EditSourceImage,
  ImageBackground,
  ImageInputFidelity,
  ImageModeration,
  ImageModel,
  ImageOutputFormat,
  ImageQuality,
} from "../../types";
import OptimizePromptModal from "./OptimizePromptModal";

const qualityOptions: ImageQuality[] = ["auto", "high", "medium", "low"];
const outputFormatOptions: ImageOutputFormat[] = ["png", "jpeg", "webp"];
const backgroundOptions: ImageBackground[] = ["auto", "opaque", "transparent"];
const moderationOptions: ImageModeration[] = ["auto", "low"];
const inputFidelityOptions: ImageInputFidelity[] = ["high", "low"];
const imageCountOptions = [1, 2, 3, 4];

interface GenerationComposerProps {
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  prompt: string;
  imageModel: ImageModel;
  quality: ImageQuality;
  background: ImageBackground;
  outputFormat: ImageOutputFormat;
  moderation: ImageModeration;
  inputFidelity: ImageInputFidelity;
  imageCount: number;
  editSources: EditSourceImage[];
  editingPromptMessageId: string | null;
  onPromptChange: (value: string) => void;
  onModelChange: (model: ImageModel) => void;
  onQualityChange: (quality: ImageQuality) => void;
  onBackgroundChange: (background: ImageBackground) => void;
  onOutputFormatChange: (outputFormat: ImageOutputFormat) => void;
  onModerationChange: (moderation: ImageModeration) => void;
  onInputFidelityChange: (inputFidelity: ImageInputFidelity) => void;
  onImageCountChange: (imageCount: number) => void;
  onAddUploadedSources: () => void;
  onClearEditSources: () => void;
  onRemoveEditSource: (sourceId: string) => void;
  onCancelPromptEdit: () => void;
  onGenerate: () => void;
  isGenerating?: boolean;
}

export default function GenerationComposer({
  textareaRef,
  prompt,
  imageModel,
  quality,
  background,
  outputFormat,
  moderation,
  inputFidelity,
  imageCount,
  editSources,
  editingPromptMessageId,
  onPromptChange,
  onModelChange,
  onQualityChange,
  onBackgroundChange,
  onOutputFormatChange,
  onModerationChange,
  onInputFidelityChange,
  onImageCountChange,
  onAddUploadedSources,
  onClearEditSources,
  onRemoveEditSource,
  onCancelPromptEdit,
  onGenerate,
  isGenerating = false,
}: GenerationComposerProps) {
  const { t } = useTranslation();

  // ── Optimize state ──────────────────────────────────────────────────────────
  const [optimizeError, setOptimizeError] = useState<string | null>(null);
  const [optimizeOriginalPrompt, setOptimizeOriginalPrompt] = useState("");
  const [optimizedPrompt, setOptimizedPrompt] = useState("");
  const [showOptimizeModal, setShowOptimizeModal] = useState(false);
  const [selectedConfigId, setSelectedConfigId] = useState<string>("");
  const [selectedMultimodalConfigId, setSelectedMultimodalConfigId] = useState<string>("");
  const [showConfigDropdown, setShowConfigDropdown] = useState(false);

  const { data: llmConfigs = [] } = useLlmConfigsQuery();

  const enabledTextConfigs = useMemo(
    () => llmConfigs.filter((c) => c.enabled && c.capability === "text"),
    [llmConfigs],
  );

  const enabledMultimodalConfigs = useMemo(
    () => llmConfigs.filter((c) => c.enabled && c.capability === "multimodal"),
    [llmConfigs],
  );

  const optimizeMutation = useOptimizePromptMutation();

  // Auto-select single config; clear selection if chosen config is removed
  useEffect(() => {
    if (enabledTextConfigs.length === 1) {
      setSelectedConfigId(enabledTextConfigs[0].id);
    } else if (enabledTextConfigs.length === 0) {
      setSelectedConfigId("");
    } else if (
      selectedConfigId &&
      !enabledTextConfigs.find((c) => c.id === selectedConfigId)
    ) {
      setSelectedConfigId("");
    }
  }, [enabledTextConfigs, selectedConfigId]);

  // Auto-select single multimodal config; clear selection if chosen config is removed
  useEffect(() => {
    if (enabledMultimodalConfigs.length === 1) {
      setSelectedMultimodalConfigId(enabledMultimodalConfigs[0].id);
    } else if (enabledMultimodalConfigs.length === 0) {
      setSelectedMultimodalConfigId("");
    } else if (
      selectedMultimodalConfigId &&
      !enabledMultimodalConfigs.find((c) => c.id === selectedMultimodalConfigId)
    ) {
      setSelectedMultimodalConfigId("");
    }
  }, [enabledMultimodalConfigs, selectedMultimodalConfigId]);

  // Clear error when prompt changes
  useEffect(() => {
    setOptimizeError(null);
  }, [prompt]);

  // Close config dropdown on click outside
  const configDropdownRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!showConfigDropdown) return;
    const handleClick = (e: MouseEvent) => {
      if (
        configDropdownRef.current &&
        !configDropdownRef.current.contains(e.target as Node)
      ) {
        setShowConfigDropdown(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [showConfigDropdown]);

  const hasEditSources = editSources.length > 0;

  const effectiveConfigId = hasEditSources
    ? enabledMultimodalConfigs.length === 1
      ? enabledMultimodalConfigs[0].id
      : selectedMultimodalConfigId
    : enabledTextConfigs.length === 1
      ? enabledTextConfigs[0].id
      : selectedConfigId;

  const canOptimize = hasEditSources
    ? enabledMultimodalConfigs.length > 0 &&
      prompt.trim().length > 0 &&
      !!effectiveConfigId &&
      !optimizeMutation.isPending &&
      !isGenerating
    : enabledTextConfigs.length > 0 &&
      prompt.trim().length > 0 &&
      !!effectiveConfigId &&
      !optimizeMutation.isPending &&
      !isGenerating;

  const handleOptimize = useCallback(async () => {
    const trimmed = prompt.trim();
    if (!trimmed || !effectiveConfigId) return;

    setOptimizeError(null);
    setOptimizeOriginalPrompt(trimmed);

    const imagePaths =
      hasEditSources && editSources.length > 0
        ? editSources.slice(0, 3).map((s) => s.path)
        : undefined;

    try {
      const result = await optimizeMutation.mutateAsync({
        prompt: trimmed,
        configId: effectiveConfigId,
        imagePaths,
      });
      setOptimizedPrompt(result);
      setShowOptimizeModal(true);
    } catch (e) {
      setOptimizeError(e instanceof Error ? e.message : String(e));
    }
  }, [prompt, effectiveConfigId, hasEditSources, editSources, optimizeMutation]);

  const handleUseOptimized = useCallback(() => {
    onPromptChange(optimizedPrompt);
    setShowOptimizeModal(false);
    setOptimizedPrompt("");
    setOptimizeOriginalPrompt("");
  }, [optimizedPrompt, onPromptChange]);

  const handleKeepOriginal = useCallback(() => {
    setShowOptimizeModal(false);
    setOptimizedPrompt("");
    setOptimizeOriginalPrompt("");
  }, []);

  // ── Model catalog ───────────────────────────────────────────────────────────
  const modelCatalogEntry = getImageModelCatalogEntry(imageModel);
  const { parameterCapabilities } = modelCatalogEntry;
  const showQuality = parameterCapabilities.qualities.length > 1;
  const showBackground = parameterCapabilities.backgrounds.length > 1;
  const showImageCount = parameterCapabilities.imageCounts.length > 0;
  const showOutputFormat = parameterCapabilities.outputFormats.length > 1;
  const showModeration = parameterCapabilities.moderationLevels.length > 1;
  const showSourceEditing = modelCatalogEntry.supportsEdit;
  const showInputFidelity =
    showSourceEditing &&
    editSources.length > 0 &&
    parameterCapabilities.inputFidelityOptions.length > 1;
  const parameterColumnCount = [
    true,
    showQuality,
    showBackground,
    showImageCount,
    showOutputFormat,
    showModeration,
    showInputFidelity,
  ].filter(Boolean).length;

  return (
    <>
      <div className="border-t border-border-subtle bg-subtle/30 px-4 py-2.5 sm:px-6">
        <div
          role="toolbar"
          aria-label={t("generate.parametersLabel")}
          className="w-full overflow-hidden"
        >
          <div
            data-testid="generation-parameter-row"
            className="grid w-full min-w-0 grid-rows-1 items-center gap-1.5 whitespace-nowrap"
            style={{
              gridTemplateColumns: `repeat(${parameterColumnCount}, minmax(0, 1fr))`,
            }}
          >
            <SelectField
              label={t("generate.modelLabel")}
              value={imageModel}
              onChange={(value) => onModelChange(value as ImageModel)}
              options={IMAGE_MODEL_CATALOG.map((entry) => ({
                value: entry.id,
                label: t(entry.i18nKey),
              }))}
            />
            {showQuality && (
              <SelectField
                label={t("generate.qualityLabel")}
                value={quality}
                onChange={(value) => onQualityChange(value as ImageQuality)}
                options={qualityOptions
                  .filter((value) =>
                    parameterCapabilities.qualities.includes(value),
                  )
                  .map((value) => ({
                    value,
                    label: t(`generate.quality.${value}`),
                  }))}
              />
            )}
            {showBackground && (
              <SelectField
                label={t("generate.backgroundLabel")}
                value={background}
                onChange={(value) =>
                  onBackgroundChange(value as ImageBackground)
                }
                options={backgroundOptions
                  .filter((value) =>
                    parameterCapabilities.backgrounds.includes(value),
                  )
                  .map((value) => ({
                    value,
                    label: t(`generate.background.${value}`),
                    disabled: outputFormat === "jpeg" && value === "transparent",
                  }))}
              />
            )}
            {showImageCount && (
              <SelectField
                label={t("generate.countLabel")}
                value={String(imageCount)}
                onChange={(value) => onImageCountChange(Number(value))}
                options={imageCountOptions
                  .filter((value) =>
                    parameterCapabilities.imageCounts.includes(value),
                  )
                  .map((value) => ({
                    value: String(value),
                    label: t("generate.countValue", { count: value }),
                  }))}
              />
            )}
            {showOutputFormat && (
              <SelectField
                label={t("generate.formatLabel")}
                value={outputFormat}
                onChange={(value) =>
                  onOutputFormatChange(value as ImageOutputFormat)
                }
                options={outputFormatOptions
                  .filter((value) =>
                    parameterCapabilities.outputFormats.includes(value),
                  )
                  .map((value) => ({
                    value,
                    label: t(`generate.format.${value}`),
                    disabled: background === "transparent" && value === "jpeg",
                  }))}
              />
            )}
            {showModeration && (
              <SelectField
                label={t("generate.moderationLabel")}
                value={moderation}
                onChange={(value) => onModerationChange(value as ImageModeration)}
                options={moderationOptions
                  .filter((value) =>
                    parameterCapabilities.moderationLevels.includes(value),
                  )
                  .map((value) => ({
                    value,
                    label: t(`generate.moderation.${value}`),
                  }))}
              />
            )}
            {showInputFidelity && (
              <SelectField
                label={t("generate.inputFidelityLabel")}
                value={inputFidelity}
                onChange={(value) =>
                  onInputFidelityChange(value as ImageInputFidelity)
                }
                options={inputFidelityOptions
                  .filter((value) =>
                    parameterCapabilities.inputFidelityOptions.includes(value),
                  )
                  .map((value) => ({
                    value,
                    label: t(`generate.inputFidelity.${value}`),
                  }))}
              />
            )}
          </div>
        </div>
      </div>

      <div className="bg-surface px-6 pt-4 pb-5">
        <div className="w-full">
          <div className="relative rounded-[18px] border border-border-subtle bg-subtle/40 p-3 transition-all duration-200 focus-within:border-primary/40 focus-within:bg-surface focus-within:shadow-[0_0_0_4px_rgba(79,106,255,0.1)]">
            {editingPromptMessageId && (
              <div className="mb-3 flex items-center justify-between gap-3 rounded-[12px] border border-primary/12 bg-primary/6 px-3 py-2">
                <div className="text-[12px] font-medium text-foreground/80">
                  {t("generate.editingPrompt")}
                </div>
                <button
                  onClick={onCancelPromptEdit}
                  className="text-[12px] font-medium text-primary transition-colors hover:text-primary/80"
                >
                  {t("generate.cancelEditPrompt")}
                </button>
              </div>
            )}

            <div className="mb-3 flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                {showSourceEditing && (
                  <button
                    onClick={onAddUploadedSources}
                    className="inline-flex items-center gap-2 rounded-[10px] border border-border-subtle bg-surface px-3 py-2 text-[12px] font-medium text-foreground/80 transition-colors hover:border-border hover:text-foreground"
                  >
                    <ImagePlus size={14} />
                    {t("generate.uploadSource")}
                  </button>
                )}
              </div>

              {editSources.length > 0 && (
                <button
                  onClick={onClearEditSources}
                  className="text-[12px] font-medium text-muted transition-colors hover:text-foreground"
                >
                  {t("generate.clearSources")}
                </button>
              )}
            </div>

            {editSources.length > 0 && (
              <div className="mb-3">
                <div className="mb-2 flex items-center gap-2 text-[12px] font-medium text-foreground/80">
                  <Wand2 size={14} className="text-primary" />
                  {t("generate.editingSources", { count: editSources.length })}
                </div>
                <div className="flex flex-wrap gap-2">
                  {editSources.map((source) => (
                    <div
                      key={source.id}
                      className="relative overflow-hidden rounded-[12px] border border-border-subtle bg-surface"
                    >
                      <img
                        src={toAssetUrl(source.path)}
                        alt={source.label}
                        className="h-20 w-20 object-cover"
                      />
                      <button
                        onClick={() => onRemoveEditSource(source.id)}
                        className="absolute right-1 top-1 flex h-6 w-6 items-center justify-center rounded-full bg-black/60 text-white transition-colors hover:bg-black/80"
                        title={t("generate.removeSource")}
                      >
                        <X size={12} />
                      </button>
                      <div className="max-w-20 truncate px-2 py-1 text-[10px] text-muted">
                        {source.label}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            <textarea
              ref={textareaRef}
              value={prompt}
              onChange={(e) => onPromptChange(e.target.value)}
              placeholder={
                editSources.length > 0
                  ? t("generate.editPlaceholder")
                  : t("generate.placeholder")
              }
              rows={2}
              className="w-full resize-none border-none bg-transparent text-[14px] leading-[1.6] text-foreground placeholder:text-muted/50 focus:outline-none pr-[190px]"
            />
            <div className="absolute right-3 bottom-3 flex items-center gap-2">
              {/* Config selector — visible only when multiple enabled configs of the active type */}
              {hasEditSources
                ? enabledMultimodalConfigs.length > 1 && (
                    <div className="relative" ref={configDropdownRef}>
                      <button
                        type="button"
                        onClick={() => setShowConfigDropdown(!showConfigDropdown)}
                        className="flex h-[34px] items-center gap-1 rounded-[9px] border border-border-subtle bg-surface px-2 text-[11px] font-medium text-foreground/80 transition-all hover:border-border hover:text-foreground"
                      >
                        <span className="max-w-[90px] truncate">
                          {selectedMultimodalConfigId
                            ? enabledMultimodalConfigs.find(
                                (c) => c.id === selectedMultimodalConfigId,
                              )?.name ?? t("generate.llm.selectLlm")
                            : t("generate.llm.selectLlm")}
                        </span>
                        <ChevronDown size={11} className="text-muted/60" />
                      </button>
                      {showConfigDropdown && (
                        <div className="absolute bottom-full right-0 z-10 mb-1 min-w-[140px] overflow-hidden rounded-[10px] border border-border-subtle bg-surface shadow-float">
                          {enabledMultimodalConfigs.map((config) => (
                            <button
                              key={config.id}
                              type="button"
                              onClick={() => {
                                setSelectedMultimodalConfigId(config.id);
                                setShowConfigDropdown(false);
                              }}
                              className={`w-full px-3 py-2 text-left text-[11px] font-medium transition-colors hover:bg-subtle ${
                                selectedMultimodalConfigId === config.id
                                  ? "text-primary"
                                  : "text-foreground/80"
                              }`}
                            >
                              {config.name || t("generate.llm.untitled")}
                            </button>
                          ))}
                        </div>
                      )}
                    </div>
                  )
                : enabledTextConfigs.length > 1 && (
                    <div className="relative" ref={configDropdownRef}>
                      <button
                        type="button"
                        onClick={() => setShowConfigDropdown(!showConfigDropdown)}
                        className="flex h-[34px] items-center gap-1 rounded-[9px] border border-border-subtle bg-surface px-2 text-[11px] font-medium text-foreground/80 transition-all hover:border-border hover:text-foreground"
                      >
                        <span className="max-w-[90px] truncate">
                          {selectedConfigId
                            ? enabledTextConfigs.find((c) => c.id === selectedConfigId)
                                ?.name ?? t("generate.llm.selectLlm")
                            : t("generate.llm.selectLlm")}
                        </span>
                        <ChevronDown size={11} className="text-muted/60" />
                      </button>
                      {showConfigDropdown && (
                        <div className="absolute bottom-full right-0 z-10 mb-1 min-w-[140px] overflow-hidden rounded-[10px] border border-border-subtle bg-surface shadow-float">
                          {enabledTextConfigs.map((config) => (
                            <button
                              key={config.id}
                              type="button"
                              onClick={() => {
                                setSelectedConfigId(config.id);
                                setShowConfigDropdown(false);
                              }}
                              className={`w-full px-3 py-2 text-left text-[11px] font-medium transition-colors hover:bg-subtle ${
                                selectedConfigId === config.id
                                  ? "text-primary"
                                  : "text-foreground/80"
                              }`}
                            >
                              {config.name || t("generate.llm.untitled")}
                            </button>
                          ))}
                        </div>
                      )}
                    </div>
                  )}

              {/* Optimize button */}
              <motion.button
                type="button"
                onClick={handleOptimize}
                disabled={!canOptimize}
                aria-label={t("generate.llm.optimize")}
                title={
                  hasEditSources
                    ? t("generate.llm.optimizeWithImages")
                    : t("generate.llm.optimizeTitle")
                }
                whileHover={{ scale: 1.02 }}
                whileTap={{ scale: 0.97 }}
                className="flex items-center justify-center gap-1.5 rounded-[10px] border border-primary/15 bg-primary/5 px-3 py-2 text-[12px] font-medium text-primary/80 transition-all hover:border-primary/25 hover:bg-primary/10 hover:text-primary disabled:pointer-events-none disabled:opacity-30"
              >
                {optimizeMutation.isPending ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Sparkles size={14} />
                )}
                <span className="hidden sm:inline">{t("generate.llm.optimize")}</span>
              </motion.button>

              {/* Send button */}
              <motion.button
                onClick={onGenerate}
                disabled={!prompt.trim() || optimizeMutation.isPending}
                aria-label={t("generate.submit")}
                whileHover={{ scale: 1.02, y: -1 }}
                whileTap={{ scale: 0.97 }}
                className="flex items-center gap-2 rounded-[12px] gradient-primary px-5 py-2.5 text-[13px] font-semibold text-white shadow-[0_4px_12px_rgba(79,106,255,0.3)] transition-shadow hover:shadow-[0_6px_16px_rgba(79,106,255,0.4)] disabled:opacity-40 disabled:pointer-events-none disabled:shadow-none"
              >
                <ArrowUp size={15} strokeWidth={2.5} />
              </motion.button>
            </div>

            {/* Inline optimize error */}
            {optimizeError && (
              <div className="mt-2 rounded-[8px] border border-error/15 bg-error/5 px-3 py-1.5 text-[11px] text-error">
                {optimizeError}
              </div>
            )}
          </div>
        </div>
      </div>

      <AnimatePresence>
        {showOptimizeModal && (
          <OptimizePromptModal
            open={showOptimizeModal}
            originalPrompt={optimizeOriginalPrompt}
            optimizedPrompt={optimizedPrompt}
            onUseOptimized={handleUseOptimized}
            onKeepOriginal={handleKeepOriginal}
            onOptimizedChange={setOptimizedPrompt}
          />
        )}
      </AnimatePresence>
    </>
  );
}

interface SelectFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: Array<{ value: string; label: string; disabled?: boolean }>;
}

function SelectField({ label, value, onChange, options }: SelectFieldProps) {
  return (
    <label className="flex h-[34px] min-w-0 items-center gap-1 rounded-[10px] border border-border-subtle bg-surface px-2 text-[12px] text-foreground transition-colors focus-within:border-border">
      <span
        className="max-w-[58px] shrink truncate text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60"
        title={label}
      >
        {label}
      </span>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="select-control min-w-0 flex-1 truncate bg-transparent pr-8 text-[12px] font-medium text-foreground focus:outline-none"
      >
        {options.map((option) => (
          <option
            key={option.value}
            value={option.value}
            disabled={option.disabled}
          >
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}
