import { useState, useCallback, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  editImage,
  generateImage,
  getConversationGenerations,
  getImageModel,
  saveImageModel,
  deleteGeneration,
  messageImageToEditSource,
  pickSourceImages,
  toAssetUrl,
} from "../lib/api";
import { consumePendingEditSources } from "../lib/editSources";
import { useLayoutContext } from "../components/layout/AppLayout";
import ConfirmDialog from "../components/common/ConfirmDialog";
import MessageBubble from "../components/generate/MessageBubble";
import Lightbox from "../components/lightbox/Lightbox";
import FolderSelector from "../components/favorites/FolderSelector";
import {
  IMAGE_MODEL_CATALOG,
  getImageModelCatalogEntry,
} from "../lib/modelCatalog";
import type {
  EditSourceImage,
  ImageBackground,
  ImageInputFidelity,
  ImageModeration,
  ImageOutputFormat,
  ImageModel,
  ImageQuality,
  ImageSize,
  Message,
  MessageImage,
  GenerationResult,
  GenerationParams,
  RetryGenerationRequest,
} from "../types";
import { useTranslation } from "react-i18next";
import { Image as ImageIcon, ArrowUp, ImagePlus, X, Wand2 } from "lucide-react";

const sizes: { value: ImageSize; label: string; descKey: string }[] = [
  { value: "auto", label: "Auto", descKey: "generate.auto" },
  { value: "1024x1024", label: "1:1", descKey: "generate.square" },
  { value: "1536x1024", label: "3:2", descKey: "generate.landscape" },
  { value: "1024x1536", label: "2:3", descKey: "generate.portrait" },
];

const qualityOptions: ImageQuality[] = ["auto", "high", "medium", "low"];
const outputFormatOptions: ImageOutputFormat[] = ["png", "jpeg", "webp"];
const backgroundOptions: ImageBackground[] = ["auto", "opaque", "transparent"];
const moderationOptions: ImageModeration[] = ["auto", "low"];
const inputFidelityOptions: ImageInputFidelity[] = ["high", "low"];
const imageCountOptions = [1, 2, 3, 4];
const DEFAULT_IMAGE_MODEL: ImageModel = "gpt-image-2";
const DEFAULT_IMAGE_MODEL_ENTRY = getImageModelCatalogEntry(DEFAULT_IMAGE_MODEL);

function modelSupportsEdit(model: ImageModel): boolean {
  return getImageModelCatalogEntry(model).supportsEdit;
}

function generationsToMessages(generations: GenerationResult[]): Message[] {
  const messages: Message[] = [];
  for (const gr of generations) {
    const images: MessageImage[] = gr.images.map((img) => ({
      imageId: img.id,
      generationId: img.generation_id,
      path: img.file_path,
      thumbnailPath: img.thumbnail_path,
    }));
    messages.push({
      id: `user-${gr.generation.id}`,
      role: "user",
      content: gr.generation.prompt,
      sourceImages: [],
      status: "complete",
      createdAt: gr.generation.created_at,
    });
    messages.push({
      id: `assistant-${gr.generation.id}`,
      role: "assistant",
      content: "",
      generationId: gr.generation.id,
      images,
      error: gr.generation.error_message ?? undefined,
      status:
        gr.generation.status === "completed"
          ? "complete"
          : gr.generation.status === "failed"
            ? "failed"
            : "processing",
      createdAt: gr.generation.created_at,
    });
  }
  return messages;
}

export default function GeneratePage() {
  const { t } = useTranslation();
  const {
    activeConversationId,
    setActiveConversationId,
    refreshConversations,
  } = useLayoutContext();
  const [messages, setMessages] = useState<Message[]>([]);
  const [prompt, setPrompt] = useState("");
  const [size, setSize] = useState<ImageSize>(
    DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.size,
  );
  const [quality, setQuality] = useState<ImageQuality>(
    DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.quality,
  );
  const [background, setBackground] = useState<ImageBackground>(
    DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.background,
  );
  const [outputFormat, setOutputFormat] = useState<ImageOutputFormat>(
    DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.outputFormat,
  );
  const [moderation, setModeration] = useState<ImageModeration>(
    DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.moderation,
  );
  const [inputFidelity, setInputFidelity] =
    useState<ImageInputFidelity>(
      DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.inputFidelity,
    );
  const [imageCount, setImageCount] = useState(
    DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.imageCount,
  );
  const [imageModel, setImageModel] = useState<ImageModel>(DEFAULT_IMAGE_MODEL);
  const [editSources, setEditSources] = useState<EditSourceImage[]>([]);
  const [editingPromptMessageId, setEditingPromptMessageId] = useState<
    string | null
  >(null);
  const [lightboxState, setLightboxState] = useState<{
    images: MessageImage[];
    index: number;
  } | null>(null);
  const [folderSelectorImageId, setFolderSelectorImageId] = useState<
    string | null
  >(null);
  const [pendingDeleteGenerationId, setPendingDeleteGenerationId] = useState<
    string | null
  >(null);
  const [isDeletingGeneration, setIsDeletingGeneration] = useState(false);
  const [chatViewportHeight, setChatViewportHeight] = useState(0);

  const scrollRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const autoScrollRef = useRef(true);
  const imageModelRef = useRef(imageModel);
  const didUserSelectModelRef = useRef(false);
  const hasComposerDraftRef = useRef(false);

  const setActiveImageModel = useCallback((model: ImageModel) => {
    imageModelRef.current = model;
    setImageModel(model);
  }, []);

  const markComposerDraftStarted = useCallback(() => {
    hasComposerDraftRef.current = true;
  }, []);

  const applyModelDefaults = useCallback((model: ImageModel) => {
    const { parameterDefaults, supportsEdit } = getImageModelCatalogEntry(model);

    setSize(parameterDefaults.size);
    setQuality(parameterDefaults.quality);
    setBackground(parameterDefaults.background);
    setOutputFormat(parameterDefaults.outputFormat);
    setModeration(parameterDefaults.moderation);
    setInputFidelity(parameterDefaults.inputFidelity);
    setImageCount(parameterDefaults.imageCount);

    if (!supportsEdit) {
      setEditSources([]);
    }
  }, []);

  const reconcileDraftStateForModel = useCallback((model: ImageModel) => {
    const {
      parameterDefaults,
      parameterCapabilities,
      supportsEdit,
    } = getImageModelCatalogEntry(model);

    setSize((current) =>
      parameterCapabilities.sizes.includes(current)
        ? current
        : parameterDefaults.size,
    );
    setQuality((current) =>
      parameterCapabilities.qualities.includes(current)
        ? current
        : parameterDefaults.quality,
    );
    setBackground((current) =>
      parameterCapabilities.backgrounds.includes(current)
        ? current
        : parameterDefaults.background,
    );
    setOutputFormat((current) =>
      parameterCapabilities.outputFormats.includes(current)
        ? current
        : parameterDefaults.outputFormat,
    );
    setModeration((current) =>
      parameterCapabilities.moderationLevels.includes(current)
        ? current
        : parameterDefaults.moderation,
    );
    setImageCount((current) =>
      parameterCapabilities.imageCounts.includes(current)
        ? current
        : parameterDefaults.imageCount,
    );
    setInputFidelity((current) =>
      supportsEdit && parameterCapabilities.inputFidelityOptions.includes(current)
        ? current
        : parameterDefaults.inputFidelity,
    );

    if (!supportsEdit) {
      setEditSources([]);
    }
  }, []);

  const loadConversationMessages = useCallback(async (conversationId: string) => {
    const generations = await getConversationGenerations(conversationId);
    setMessages(generationsToMessages(generations));
  }, []);

  useEffect(() => {
    if (!activeConversationId) {
      setMessages([]);
      return;
    }
    loadConversationMessages(activeConversationId).catch(() => {});
  }, [activeConversationId, loadConversationMessages]);

  useEffect(() => {
    if (autoScrollRef.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  useEffect(() => {
    const scrollElement = scrollRef.current;
    if (!scrollElement) return;

    const updateChatViewportHeight = () => {
      setChatViewportHeight(scrollElement.clientHeight);
    };

    updateChatViewportHeight();

    if (typeof ResizeObserver === "undefined") {
      return;
    }

    const resizeObserver = new ResizeObserver(() => {
      updateChatViewportHeight();
    });
    resizeObserver.observe(scrollElement);

    return () => {
      resizeObserver.disconnect();
    };
  }, []);

  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    autoScrollRef.current = scrollHeight - scrollTop - clientHeight < 100;
  }, []);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height =
        Math.min(textareaRef.current.scrollHeight, 120) + "px";
    }
  }, [prompt]);

  useEffect(() => {
    const pendingSources = consumePendingEditSources();
    if (pendingSources.length > 0) {
      markComposerDraftStarted();
      setEditSources((current) => mergeEditSources(current, pendingSources));
    }
  }, [markComposerDraftStarted]);

  useEffect(() => {
    let cancelled = false;

    getImageModel().then((model) => {
      if (cancelled || didUserSelectModelRef.current) {
        return;
      }

      setActiveImageModel(model);

      if (!hasComposerDraftRef.current) {
        applyModelDefaults(model);
      } else {
        reconcileDraftStateForModel(model);
      }
    }).catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [applyModelDefaults, reconcileDraftStateForModel, setActiveImageModel]);

  const handleAddUploadedSources = useCallback(async () => {
    markComposerDraftStarted();
    const paths = await pickSourceImages();
    if (paths.length === 0) return;
    if (!getImageModelCatalogEntry(imageModelRef.current).supportsEdit) {
      textareaRef.current?.focus();
      return;
    }

    setEditSources((current) =>
      mergeEditSources(
        current,
        paths.map((path) => createUploadedEditSource(path)),
      ),
    );
    textareaRef.current?.focus();
  }, [markComposerDraftStarted]);

  const handleBackgroundChange = useCallback(
    (value: ImageBackground) => {
      markComposerDraftStarted();
      setBackground(value);
      if (value === "transparent" && outputFormat === "jpeg") {
        setOutputFormat("png");
      }
    },
    [markComposerDraftStarted, outputFormat],
  );

  const handleOutputFormatChange = useCallback(
    (value: ImageOutputFormat) => {
      markComposerDraftStarted();
      setOutputFormat(value);
      if (value === "jpeg" && background === "transparent") {
        setBackground("auto");
      }
    },
    [background, markComposerDraftStarted],
  );

  const handleUseImageAsSource = useCallback((image: MessageImage) => {
    markComposerDraftStarted();
    if (!modelSupportsEdit(imageModelRef.current)) {
      textareaRef.current?.focus();
      return;
    }
    setEditSources((current) =>
      mergeEditSources(current, [messageImageToEditSource(image)]),
    );
    textareaRef.current?.focus();
  }, [markComposerDraftStarted]);

  const handleModelChange = useCallback((model: ImageModel) => {
    didUserSelectModelRef.current = true;
    markComposerDraftStarted();
    setActiveImageModel(model);
    applyModelDefaults(model);
    saveImageModel(model).catch(() => {});
  }, [applyModelDefaults, markComposerDraftStarted, setActiveImageModel]);

  function buildGenerationParams(request: RetryGenerationRequest): GenerationParams {
    const capabilities = getImageModelCatalogEntry(request.model).parameterCapabilities;

    return {
      prompt: request.prompt,
      model: request.model,
      ...(capabilities.sizes.length > 0 ? { size: request.size } : {}),
      ...(capabilities.qualities.length > 1 ? { quality: request.quality } : {}),
      ...(capabilities.backgrounds.length > 1 ? { background: request.background } : {}),
      ...(capabilities.outputFormats.length > 1
        ? { outputFormat: request.outputFormat }
        : {}),
      ...(capabilities.moderationLevels.length > 1
        ? { moderation: request.moderation }
        : {}),
      ...(capabilities.imageCounts.length > 0
        ? { imageCount: request.imageCount }
        : {}),
      conversationId: request.conversationId,
    };
  }

  function buildEditParams(request: RetryGenerationRequest): GenerationParams & {
    sourceImagePaths: string[];
    inputFidelity?: ImageInputFidelity;
  } {
    const capabilities = getImageModelCatalogEntry(request.model).parameterCapabilities;

    return {
      ...buildGenerationParams(request),
      sourceImagePaths: request.editSources.map((source) => source.path),
      ...(capabilities.inputFidelityOptions.length > 1
        ? { inputFidelity: request.inputFidelity }
        : {}),
    };
  }

  const handleRemoveEditSource = useCallback((sourceId: string) => {
    markComposerDraftStarted();
    setEditSources((current) =>
      current.filter((source) => source.id !== sourceId),
    );
  }, [markComposerDraftStarted]);

  const submitGenerationRequest = useCallback(
    async (request: RetryGenerationRequest) => {
      const promptText = request.prompt.trim();
      if (!promptText) return;

      const tempId = crypto.randomUUID();
      const retryRequest: RetryGenerationRequest = {
        ...request,
        prompt: promptText,
        editSources: request.editSources.map((source) => ({ ...source })),
      };
      const normalizedRequest: RetryGenerationRequest = {
        ...retryRequest,
        editSources: modelSupportsEdit(retryRequest.model)
          ? retryRequest.editSources
          : [],
      };
      const userMsg: Message = {
        id: `user-${tempId}`,
        role: "user",
        content: promptText,
        sourceImages: editSourcesToMessageImages(
          normalizedRequest.editSources,
          tempId,
        ),
        status: "complete",
        createdAt: new Date().toISOString(),
      };
      const assistantMsg: Message = {
        id: `assistant-${tempId}`,
        role: "assistant",
        content: "",
        generationId: tempId,
        status: "processing",
        retryRequest: normalizedRequest,
        createdAt: new Date().toISOString(),
      };
      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      autoScrollRef.current = true;

      try {
        const result =
          normalizedRequest.editSources.length > 0
            ? await editImage(buildEditParams(normalizedRequest))
            : await generateImage(buildGenerationParams(normalizedRequest));
        setMessages((prev) =>
          prev.map((m) =>
            m.id === `assistant-${tempId}`
              ? {
                  ...m,
                  id: `assistant-${result.generation_id}`,
                  generationId: result.generation_id,
                  images: result.images.map((img) => ({
                    imageId: img.id,
                    generationId: img.generation_id,
                    path: img.file_path,
                    thumbnailPath: img.thumbnail_path,
                  })),
                  status: "complete" as const,
                }
              : m,
          ),
        );
        setEditSources([]);
        setActiveConversationId(result.conversation_id);
        await loadConversationMessages(result.conversation_id);
        refreshConversations();
      } catch (e) {
        setMessages((prev) =>
          prev.map((m) =>
            m.id === `assistant-${tempId}`
              ? {
                  ...m,
                  status: "failed" as const,
                  error: String(e),
                  retryRequest: normalizedRequest,
                }
              : m,
          ),
        );
        refreshConversations();
      }
    },
    [loadConversationMessages, refreshConversations, setActiveConversationId],
  );

  async function handleGenerate() {
    const promptText = prompt.trim();
    if (!promptText) return;

    setPrompt("");
    setEditingPromptMessageId(null);
    await submitGenerationRequest({
      prompt: promptText,
      model: imageModel,
      size,
      quality,
      background,
      outputFormat,
      moderation,
      inputFidelity,
      imageCount,
      conversationId: activeConversationId,
      editSources: editSources.map((source) => ({ ...source })),
    });
  }

  const handleImageClick = useCallback(
    (images: MessageImage[], index: number) => {
      setLightboxState({ images, index });
    },
    [],
  );

  const handleDeleteFromBubble = useCallback(
    async (generationId: string) => {
      setIsDeletingGeneration(true);
      try {
        await deleteGeneration(generationId);
        setMessages((prev) =>
          prev.filter(
            (m) =>
              m.generationId !== generationId &&
              m.id !== `user-${generationId}`,
          ),
        );
        setLightboxState((current) => {
          if (!current) return null;
          return current.images.some(
            (image) => image.generationId === generationId,
          )
            ? null
            : current;
        });
        refreshConversations();
      } finally {
        setIsDeletingGeneration(false);
        setPendingDeleteGenerationId(null);
      }
    },
    [refreshConversations],
  );

  const handleRequestDeleteGeneration = useCallback((generationId: string) => {
    setPendingDeleteGenerationId(generationId);
  }, []);

  const handleCancelDeleteGeneration = useCallback(() => {
    if (isDeletingGeneration) return;
    setPendingDeleteGenerationId(null);
  }, [isDeletingGeneration]);

  const handleConfirmDeleteGeneration = useCallback(async () => {
    if (!pendingDeleteGenerationId || isDeletingGeneration) return;
    await handleDeleteFromBubble(pendingDeleteGenerationId);
  }, [handleDeleteFromBubble, isDeletingGeneration, pendingDeleteGenerationId]);

  const handleRetryMessage = useCallback(
    async (message: Message) => {
      if (!message.retryRequest) return;
      await submitGenerationRequest(message.retryRequest);
    },
    [submitGenerationRequest],
  );

  const handleEditPrompt = useCallback((message: Message) => {
    markComposerDraftStarted();
    setPrompt(message.content);
    setEditSources(() =>
      modelSupportsEdit(imageModelRef.current)
        ? message.sourceImages?.map((image) => messageImageToEditSource(image)) ?? []
        : [],
    );
    setEditingPromptMessageId(message.id);
    autoScrollRef.current = false;
    textareaRef.current?.focus();
  }, [markComposerDraftStarted]);

  const handleCancelPromptEdit = useCallback(() => {
    markComposerDraftStarted();
    setPrompt("");
    setEditSources([]);
    setEditingPromptMessageId(null);
    textareaRef.current?.focus();
  }, [markComposerDraftStarted]);

  const modelCatalogEntry = getImageModelCatalogEntry(imageModel);
  const { parameterCapabilities } = modelCatalogEntry;
  const showSize = parameterCapabilities.sizes.length > 0;
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
    showSize,
    showQuality,
    showBackground,
    showImageCount,
    showOutputFormat,
    showModeration,
    showInputFidelity,
  ].filter(Boolean).length;

  return (
    <div className="flex h-full flex-col">
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto"
      >
        {messages.length === 0 ? (
          <EmptyState />
        ) : (
          <div className="mx-auto max-w-[900px] space-y-7 px-6 py-6">
            <AnimatePresence initial={false}>
              {messages.map((msg) => (
                <MessageBubble
                  key={msg.id}
                  message={msg}
                  onImageClick={handleImageClick}
                  onDelete={handleRequestDeleteGeneration}
                  onEditImage={handleUseImageAsSource}
                  onEditPrompt={handleEditPrompt}
                  onFavoriteClick={setFolderSelectorImageId}
                  onRetry={(message) => void handleRetryMessage(message)}
                  chatViewportHeight={chatViewportHeight}
                />
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>

      {/* Settings bar */}
      <div className="border-t border-border-subtle bg-subtle/30 px-4 py-2.5 sm:px-6">
        <div
          role="toolbar"
          aria-label={t("generate.parametersLabel")}
          className="mx-auto w-full max-w-[900px] overflow-hidden"
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
              onChange={(value) => handleModelChange(value as ImageModel)}
              options={IMAGE_MODEL_CATALOG.map((entry) => ({
                value: entry.id,
                label: entry.label,
              }))}
            />
            {showSize && (
              <SelectField
                label={t("generate.sizeLabel")}
                value={size}
                onChange={(value) => {
                  markComposerDraftStarted();
                  setSize(value as ImageSize);
                }}
                options={sizes
                  .filter((item) => parameterCapabilities.sizes.includes(item.value))
                  .map((item) => ({
                    value: item.value,
                    label: `${item.label} · ${t(item.descKey)}`,
                  }))}
              />
            )}
            {showQuality && (
              <SelectField
                label={t("generate.qualityLabel")}
                value={quality}
                onChange={(value) => {
                  markComposerDraftStarted();
                  setQuality(value as ImageQuality);
                }}
                options={qualityOptions
                  .filter((value) => parameterCapabilities.qualities.includes(value))
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
                  handleBackgroundChange(value as ImageBackground)
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
                onChange={(value) => {
                  markComposerDraftStarted();
                  setImageCount(Number(value));
                }}
                options={imageCountOptions
                  .filter((value) => parameterCapabilities.imageCounts.includes(value))
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
                  handleOutputFormatChange(value as ImageOutputFormat)
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
                onChange={(value) => {
                  markComposerDraftStarted();
                  setModeration(value as ImageModeration);
                }}
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
                onChange={(value) => {
                  markComposerDraftStarted();
                  setInputFidelity(value as ImageInputFidelity);
                }}
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

      {/* Input area */}
      <div className="bg-surface px-6 pt-4 pb-5">
        <div className="mx-auto max-w-[900px]">
          <div className="relative rounded-[18px] border border-border-subtle bg-subtle/40 p-3 transition-all duration-200 focus-within:border-primary/40 focus-within:bg-surface focus-within:shadow-[0_0_0_4px_rgba(79,106,255,0.1)]">
            {editingPromptMessageId && (
              <div className="mb-3 flex items-center justify-between gap-3 rounded-[12px] border border-primary/12 bg-primary/6 px-3 py-2">
                <div className="text-[12px] font-medium text-foreground/80">
                  {t("generate.editingPrompt")}
                </div>
                <button
                  onClick={handleCancelPromptEdit}
                  className="text-[12px] font-medium text-primary transition-colors hover:text-primary/80"
                >
                  {t("generate.cancelEditPrompt")}
                </button>
              </div>
            )}

            <div className="mb-3 flex items-center justify-between gap-3">
              {showSourceEditing ? (
                <button
                  onClick={() => void handleAddUploadedSources()}
                  className="inline-flex items-center gap-2 rounded-[10px] border border-border-subtle bg-surface px-3 py-2 text-[12px] font-medium text-foreground/80 transition-colors hover:border-border hover:text-foreground"
                >
                  <ImagePlus size={14} />
                  {t("generate.uploadSource")}
                </button>
              ) : (
                <div />
              )}

              {editSources.length > 0 && (
                <button
                  onClick={() => {
                    markComposerDraftStarted();
                    setEditSources([]);
                  }}
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
                        onClick={() => handleRemoveEditSource(source.id)}
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
              onChange={(e) => {
                markComposerDraftStarted();
                setPrompt(e.target.value);
              }}
              placeholder={
                editSources.length > 0
                  ? t("generate.editPlaceholder")
                  : t("generate.placeholder")
              }
              rows={2}
              className="w-full resize-none border-none bg-transparent text-[14px] leading-[1.6] text-foreground placeholder:text-muted/50 focus:outline-none pr-[110px]"
            />
            <motion.button
              onClick={handleGenerate}
              disabled={!prompt.trim()}
              aria-label={t("generate.submit")}
              whileHover={{ scale: 1.02, y: -1 }}
              whileTap={{ scale: 0.97 }}
              className="absolute right-3 bottom-3 flex items-center gap-2 rounded-[12px] gradient-primary px-5 py-2.5 text-[13px] font-semibold text-white shadow-[0_4px_12px_rgba(79,106,255,0.3)] transition-shadow hover:shadow-[0_6px_16px_rgba(79,106,255,0.4)] disabled:opacity-40 disabled:pointer-events-none disabled:shadow-none"
            >
              <ArrowUp size={15} strokeWidth={2.5} />
            </motion.button>
          </div>
        </div>
      </div>

      <AnimatePresence>
        {lightboxState && (
          <Lightbox
            images={lightboxState.images}
            initialIndex={lightboxState.index}
            onClose={() => setLightboxState(null)}
            onEditImage={(image) => {
              handleUseImageAsSource(image);
              setLightboxState(null);
            }}
            onDelete={handleRequestDeleteGeneration}
          />
        )}
      </AnimatePresence>

      {folderSelectorImageId && (
        <FolderSelector
          imageId={folderSelectorImageId}
          onClose={() => setFolderSelectorImageId(null)}
        />
      )}

      <ConfirmDialog
        open={pendingDeleteGenerationId !== null}
        title={t("lightbox.deleteConfirm")}
        confirmLabel={t("favorites.confirm")}
        cancelLabel={t("favorites.cancel")}
        onConfirm={() => void handleConfirmDeleteGeneration()}
        onCancel={handleCancelDeleteGeneration}
        loading={isDeletingGeneration}
      />
    </div>
  );
}

function mergeEditSources(
  current: EditSourceImage[],
  incoming: EditSourceImage[],
): EditSourceImage[] {
  const byPath = new Map(current.map((source) => [source.path, source]));

  for (const source of incoming) {
    byPath.set(source.path, source);
  }

  return Array.from(byPath.values());
}

function createUploadedEditSource(path: string): EditSourceImage {
  const normalizedPath = path.replace(/\\/g, "/");
  const fileName = normalizedPath.split("/").pop() || "source-image";

  return {
    id: `${crypto.randomUUID()}:${normalizedPath}`,
    path,
    label: fileName,
  };
}

function editSourcesToMessageImages(
  sources: EditSourceImage[],
  generationId: string,
): MessageImage[] {
  return sources.map((source, index) => ({
    imageId: source.imageId ?? `${generationId}-source-${index}`,
    generationId: source.generationId ?? generationId,
    path: source.path,
    thumbnailPath: source.path,
  }));
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
        className="min-w-0 flex-1 truncate bg-transparent text-[12px] font-medium text-foreground focus:outline-none"
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

function EmptyState() {
  const { t } = useTranslation();
  return (
    <div className="flex h-full flex-col items-center justify-center px-6">
      <motion.div
        initial={{ opacity: 0, y: 16, filter: "blur(8px)" }}
        animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
        transition={{ duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
        className="flex flex-col items-center text-center"
      >
        <div className="relative mb-6">
          <div className="h-20 w-20 rounded-[20px] bg-gradient-to-br from-primary/8 via-lavender-light to-accent/6 flex items-center justify-center border border-border-subtle">
            <ImageIcon size={32} className="text-lavender" strokeWidth={1.4} />
          </div>
          <div className="absolute -top-1 -right-1 h-3 w-3 rounded-full bg-primary/30 animate-pulse" />
        </div>
        <p className="text-[15px] font-semibold text-foreground tracking-tight">
          {t("generate.emptyTitle")}
        </p>
        <p className="mt-2 max-w-[260px] text-[13px] leading-relaxed text-muted">
          {t("generate.emptySubtitle")}
        </p>
      </motion.div>
    </div>
  );
}
