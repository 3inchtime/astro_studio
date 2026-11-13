import { motion, AnimatePresence } from "framer-motion";
import {
  ArrowUp,
  ChevronDown,
  ImagePlus,
  Loader2,
  Sparkles,
  Wand2,
  X,
} from "lucide-react";
import type { RefObject } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toAssetUrl } from "../../lib/api";
import { getImageModelCatalogEntry } from "../../lib/modelCatalog";
import {
  useLlmConfigsQuery,
  useOptimizePromptMutation,
} from "../../lib/queries/llm";
import type {
  ComposerMode,
  ImageAdvancedOptions,
  EditSourceImage,
  ImageBackground,
  ImageInputFidelity,
  ImageModeration,
  ImageModel,
  ImageOutputFormat,
  ImageQuality,
  ImageSize,
} from "../../types";
import GenerationParameters from "./GenerationParameters";
import { createPortal } from "react-dom";
import { image26ValidationError } from "../../lib/image26Options";
import OptimizePromptModal from "./OptimizePromptModal";

export interface GenerationComposerProps {
  parametersTarget?: HTMLElement | null;
  interactionDisabled?: boolean;
  advancedOptions?: ImageAdvancedOptions;
  onAdvancedOptionsChange?: (options: ImageAdvancedOptions) => void;
  outputCompression?: number;
  onOutputCompressionChange?: (value: number) => void;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  prompt: string;
  imageModel: ImageModel;
  size?: ImageSize;
  aspectRatio?: string;
  resolution?: string;
  onSizeChange?: (size: ImageSize) => void;
  onAspectRatioChange?: (ratio: string) => void;
  onResolutionChange?: (resolution: string) => void;
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
  composerMode: ComposerMode;
  onComposerModeChange: (mode: ComposerMode) => void;
  onDeepThinkingSend: () => void;
  onPromptAgentConfigChange?: (configId: string) => void;
  isPromptAgentRunning?: boolean;
  isGenerating?: boolean;
}

export default function GenerationComposer({
  parametersTarget,
  interactionDisabled = false,
  advancedOptions = {},
  onAdvancedOptionsChange,
  outputCompression,
  onOutputCompressionChange,
  textareaRef,
  prompt,
  imageModel,
  size = "auto",
  aspectRatio,
  resolution,
  onSizeChange,
  onAspectRatioChange,
  onResolutionChange,
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
  composerMode,
  onComposerModeChange,
  onDeepThinkingSend,
  onPromptAgentConfigChange,
  isPromptAgentRunning = false,
  isGenerating = false,
}: GenerationComposerProps) {
  const { t } = useTranslation();

  // ── Optimize state ──────────────────────────────────────────────────────────
  const [optimizeError, setOptimizeError] = useState<string | null>(null);
  const [optimizeOriginalPrompt, setOptimizeOriginalPrompt] = useState("");
  const [optimizedPrompt, setOptimizedPrompt] = useState("");
  const [showOptimizeModal, setShowOptimizeModal] = useState(false);
  const [selectedConfigId, setSelectedConfigId] = useState<string>("");
  const [selectedMultimodalConfigId, setSelectedMultimodalConfigId] =
    useState<string>("");
  const [showConfigDropdown, setShowConfigDropdown] = useState(false);

  const { data: llmConfigs = [] } = useLlmConfigsQuery();

  const enabledPromptConfigs = useMemo(
    () =>
      llmConfigs.filter(
        (c) =>
          c.enabled &&
          (c.capability === "text" || c.capability === "multimodal"),
      ),
    [llmConfigs],
  );

  const enabledMultimodalConfigs = useMemo(
    () => llmConfigs.filter((c) => c.enabled && c.capability === "multimodal"),
    [llmConfigs],
  );

  const optimizeMutation = useOptimizePromptMutation();

  // Auto-select a single text prompt optimizer. Multimodal LLMs can also optimize text-only prompts.
  useEffect(() => {
    if (enabledPromptConfigs.length === 1) {
      setSelectedConfigId(enabledPromptConfigs[0].id);
    } else if (enabledPromptConfigs.length === 0) {
      setSelectedConfigId("");
    } else if (
      selectedConfigId &&
      !enabledPromptConfigs.find((c) => c.id === selectedConfigId)
    ) {
      setSelectedConfigId("");
    }
  }, [enabledPromptConfigs, selectedConfigId]);

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
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setShowConfigDropdown(false);
        configDropdownRef.current?.querySelector("button")?.focus();
      }
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [showConfigDropdown]);

  const hasEditSources = editSources.length > 0;
  const activeOptimizeConfigCount = hasEditSources
    ? enabledMultimodalConfigs.length
    : enabledPromptConfigs.length;

  const effectiveConfigId = hasEditSources
    ? enabledMultimodalConfigs.length === 1
      ? enabledMultimodalConfigs[0].id
      : selectedMultimodalConfigId
    : enabledPromptConfigs.length === 1
      ? enabledPromptConfigs[0].id
      : selectedConfigId;

  useEffect(() => {
    onPromptAgentConfigChange?.(effectiveConfigId);
  }, [effectiveConfigId, onPromptAgentConfigChange]);

  const handleOptimize = useCallback(async () => {
    const trimmed = prompt.trim();
    if (optimizeMutation.isPending) return;
    if (isGenerating) {
      setOptimizeError(t("generate.llm.generating"));
      return;
    }
    if (!trimmed) {
      setOptimizeError(t("generate.llm.emptyPrompt"));
      return;
    }
    if (activeOptimizeConfigCount === 0) {
      setOptimizeError(
        hasEditSources
          ? t("generate.llm.noMultimodalConfig")
          : t("generate.llm.noPromptConfig"),
      );
      return;
    }
    if (!effectiveConfigId) {
      setOptimizeError(t("generate.llm.selectConfigFirst"));
      setShowConfigDropdown(true);
      return;
    }

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
  }, [
    prompt,
    optimizeMutation,
    isGenerating,
    t,
    activeOptimizeConfigCount,
    hasEditSources,
    effectiveConfigId,
    editSources,
  ]);

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
  const validationError = image26ValidationError({
    model: imageModel,
    format: outputFormat,
    size,
    count: imageCount,
    background,
    editing: hasEditSources,
    options: advancedOptions,
    compression: outputCompression,
  });
  const showSourceEditing = modelCatalogEntry.supportsEdit;
  const cannotSubmit =
    interactionDisabled ||
    !!validationError ||
    !prompt.trim() ||
    isGenerating ||
    optimizeMutation.isPending ||
    isPromptAgentRunning;
  const submit =
    composerMode === "deep-thinking" ? onDeepThinkingSend : onGenerate;

  const parameters = (
    <GenerationParameters
      advancedOptions={advancedOptions}
      onAdvancedOptionsChange={onAdvancedOptionsChange}
      outputCompression={outputCompression}
      onOutputCompressionChange={onOutputCompressionChange}
      imageModel={imageModel}
      size={size}
      aspectRatio={aspectRatio}
      resolution={resolution}
      onSizeChange={onSizeChange}
      onAspectRatioChange={onAspectRatioChange}
      onResolutionChange={onResolutionChange}
      quality={quality}
      background={background}
      outputFormat={outputFormat}
      moderation={moderation}
      inputFidelity={inputFidelity}
      imageCount={imageCount}
      editSources={editSources}
      onModelChange={onModelChange}
      onQualityChange={onQualityChange}
      onBackgroundChange={onBackgroundChange}
      onOutputFormatChange={onOutputFormatChange}
      onModerationChange={onModerationChange}
      onInputFidelityChange={onInputFidelityChange}
      onImageCountChange={onImageCountChange}
    />
  );

  return (
    <>
      {parametersTarget
        ? createPortal(parameters, parametersTarget)
        : parameters}

      <div className="generation-composer shrink-0 border-t border-border-subtle bg-surface px-5 py-4">
        <div className="mx-auto w-full max-w-[980px]">
          <div
            data-testid="generation-command-surface"
            className="relative flex w-full min-h-0 flex-col gap-2"
          >
            <div className="generation-composer-scroll min-h-0 overflow-y-auto">
              {editingPromptMessageId && (
                <div className="mb-3 flex items-center justify-between gap-3 rounded-[12px] border border-primary/12 bg-primary/6 px-3 py-2">
                  <div className="text-xs font-medium text-foreground">
                    {t("generate.editingPrompt")}
                  </div>
                  <button
                    onClick={onCancelPromptEdit}
                    className="focus-ring rounded-[8px] px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-primary/8 hover:text-primary"
                  >
                    {t("generate.cancelEditPrompt")}
                  </button>
                </div>
              )}

              <div className="flex shrink-0 items-center justify-between gap-3">
                <div className="flex min-w-0 items-center gap-2">
                  <div className="flex rounded-lg bg-subtle p-0.5">
                    <button
                      type="button"
                      aria-pressed={composerMode === "direct"}
                      className={`focus-ring inline-flex min-h-8 items-center rounded-md px-3 text-xs font-medium transition-colors ${
                        composerMode === "direct"
                          ? "bg-surface text-primary shadow-button"
                          : "text-muted hover:text-foreground"
                      }`}
                      onClick={() => onComposerModeChange("direct")}
                    >
                      {t("generate.mode.direct")}
                    </button>
                    <button
                      type="button"
                      aria-pressed={composerMode === "deep-thinking"}
                      className={`focus-ring inline-flex min-h-8 items-center rounded-md px-3 text-xs font-medium transition-colors ${
                        composerMode === "deep-thinking"
                          ? "bg-surface text-primary shadow-button"
                          : "text-muted hover:text-foreground"
                      }`}
                      onClick={() => onComposerModeChange("deep-thinking")}
                    >
                      {t("generate.mode.deepThinking")}
                    </button>
                  </div>
                  {showSourceEditing && (
                    <button
                      onClick={onAddUploadedSources}
                      className="studio-control focus-ring inline-flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-medium hover:studio-control-hover"
                    >
                      <ImagePlus size={14} />
                      {t("generate.uploadSource")}
                    </button>
                  )}
                </div>

                {editSources.length > 0 && (
                  <button
                    onClick={onClearEditSources}
                    className="studio-control-subtle focus-ring rounded-lg px-2.5 py-1 text-xs font-medium hover:bg-subtle hover:text-foreground"
                  >
                    {t("generate.clearSources")}
                  </button>
                )}
              </div>

              {editSources.length > 0 && (
                <div className="min-h-0 overflow-y-auto">
                  <div className="mb-2 flex items-center gap-2 text-xs font-medium text-foreground">
                    <Wand2 size={14} className="text-primary" />
                    {t("generate.editingSources", {
                      count: editSources.length,
                    })}
                  </div>
                  <div className="flex gap-2 overflow-x-auto pb-1">
                    {editSources.map((source) => (
                      <div
                        key={source.id}
                        className="studio-card relative overflow-hidden rounded-[12px]"
                      >
                        <img
                          src={toAssetUrl(source.path)}
                          alt={source.label}
                          className="h-12 w-12 object-cover"
                        />
                        <button
                          onClick={() => onRemoveEditSource(source.id)}
                          className="focus-ring absolute right-1 top-1 flex h-6 w-6 cursor-pointer items-center justify-center rounded-lg border border-white/20 bg-black/60 text-white transition-colors hover:bg-black/80"
                          aria-label={t("generate.removeSource")}
                          title={t("generate.removeSource")}
                        >
                          <X size={12} />
                        </button>
                        <div className="max-w-20 truncate px-2 py-1 text-xs text-muted">
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
                aria-label={t("studio.prompt", { defaultValue: "Prompt" })}
                onKeyDown={(event) => {
                  if (
                    event.key !== "Enter" ||
                    !(event.metaKey || event.ctrlKey) ||
                    event.nativeEvent.isComposing ||
                    event.keyCode === 229
                  )
                    return;
                  event.preventDefault();
                  if (!cannotSubmit) submit();
                }}
                rows={2}
                className="studio-input generation-prompt block min-h-10 w-full resize-none rounded-lg bg-background px-3 py-2 text-sm leading-relaxed text-foreground placeholder:text-muted focus:border-primary focus:outline-none"
              />
            </div>
            <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
              <span className="mr-auto text-xs text-muted">
                {t("studio.submitShortcut", {
                  defaultValue: "⌘ / Ctrl + Enter to send",
                })}
              </span>
              {/* Config selector — visible only when multiple enabled configs of the active type */}
              {hasEditSources
                ? enabledMultimodalConfigs.length > 1 && (
                    <div className="relative" ref={configDropdownRef}>
                      <button
                        type="button"
                        aria-expanded={showConfigDropdown}
                        onClick={() =>
                          setShowConfigDropdown(!showConfigDropdown)
                        }
                        className="studio-control focus-ring flex h-[34px] items-center gap-1 rounded-lg px-2 text-xs font-medium hover:studio-control-hover"
                      >
                        <span className="max-w-[90px] truncate">
                          {selectedMultimodalConfigId
                            ? (enabledMultimodalConfigs.find(
                                (c) => c.id === selectedMultimodalConfigId,
                              )?.name ?? t("generate.llm.selectLlm"))
                            : t("generate.llm.selectLlm")}
                        </span>
                        <ChevronDown size={11} className="text-muted" />
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
                              className={`w-full px-3 py-2 text-left text-xs font-medium transition-colors hover:bg-subtle ${
                                selectedMultimodalConfigId === config.id
                                  ? "text-primary"
                                  : "text-foreground"
                              }`}
                            >
                              {config.name || t("generate.llm.untitled")}
                            </button>
                          ))}
                        </div>
                      )}
                    </div>
                  )
                : enabledPromptConfigs.length > 1 && (
                    <div className="relative" ref={configDropdownRef}>
                      <button
                        type="button"
                        aria-expanded={showConfigDropdown}
                        onClick={() =>
                          setShowConfigDropdown(!showConfigDropdown)
                        }
                        className="studio-control focus-ring flex h-[34px] items-center gap-1 rounded-lg px-2 text-xs font-medium hover:studio-control-hover"
                      >
                        <span className="max-w-[90px] truncate">
                          {selectedConfigId
                            ? (enabledPromptConfigs.find(
                                (c) => c.id === selectedConfigId,
                              )?.name ?? t("generate.llm.selectLlm"))
                            : t("generate.llm.selectLlm")}
                        </span>
                        <ChevronDown size={11} className="text-muted" />
                      </button>
                      {showConfigDropdown && (
                        <div className="absolute bottom-full right-0 z-10 mb-1 min-w-[140px] overflow-hidden rounded-[10px] border border-border-subtle bg-surface shadow-float">
                          {enabledPromptConfigs.map((config) => (
                            <button
                              key={config.id}
                              type="button"
                              onClick={() => {
                                setSelectedConfigId(config.id);
                                setShowConfigDropdown(false);
                              }}
                              className={`w-full px-3 py-2 text-left text-xs font-medium transition-colors hover:bg-subtle ${
                                selectedConfigId === config.id
                                  ? "text-primary"
                                  : "text-foreground"
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
                aria-busy={optimizeMutation.isPending}
                aria-label={t("generate.llm.optimize")}
                title={
                  hasEditSources
                    ? t("generate.llm.optimizeWithImages")
                    : t("generate.llm.optimizeTitle")
                }
                whileHover={{ scale: 1.02 }}
                whileTap={{ scale: 0.98 }}
                className="focus-ring flex cursor-pointer items-center justify-center gap-1.5 rounded-lg border border-primary/15 bg-primary/5 px-3 py-2 text-xs font-medium text-primary transition-all hover:border-primary/25 hover:bg-primary/10 hover:text-primary"
              >
                {optimizeMutation.isPending ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Sparkles size={14} />
                )}
                <span className="hidden sm:inline">
                  {t("generate.llm.optimize")}
                </span>
              </motion.button>

              {/* Send button */}
              <motion.button
                onClick={submit}
                disabled={cannotSubmit}
                aria-label={t("generate.submit")}
                whileHover={{ scale: 1.01 }}
                whileTap={{ scale: 0.98 }}
                className="studio-control-primary focus-ring flex h-10 items-center justify-center gap-2 rounded-lg px-4 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-50"
              >
                {isGenerating || isPromptAgentRunning ? (
                  <Loader2 size={15} className="animate-spin" />
                ) : (
                  <ArrowUp size={15} />
                )}
                {t("generate.submit")}
              </motion.button>
            </div>

            {validationError && parametersTarget && (
              <p role="alert" className="text-xs text-error">
                {t(`generate.advanced.${validationError}`)}
              </p>
            )}
            {/* Inline optimize error */}
            {optimizeError && (
              <div className="mt-2 rounded-[8px] border border-border-subtle bg-surface px-3 py-1.5 text-xs text-error">
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
