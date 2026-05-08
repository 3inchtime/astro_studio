import { useState, useCallback, useRef, useEffect, useMemo } from "react";
import { AnimatePresence } from "framer-motion";
import {
  editImage,
  generateImage,
  getConversationGenerations,
  getImageModel,
  saveImageModel,
  deleteGeneration,
  messageImageToEditSource,
  onGenerationComplete,
  onGenerationFailed,
  pickSourceImages,
} from "../lib/api";
import { consumePendingEditSources } from "../lib/editSources";
import {
  completeGenerationMessage,
  failGenerationMessage,
  generationsToMessages,
} from "../lib/generationMessages";
import {
  buildEditParams,
  buildGenerationParams,
  modelSupportsEdit,
} from "../lib/generationRequests";
import {
  createUploadedEditSource,
  editSourcesToMessageImages,
  mergeEditSources,
  normalizePromptFavorite,
} from "../lib/generatePageHelpers";
import { usePromptFavoritesQuery, useCreatePromptFavoriteMutation, useDeletePromptFavoriteMutation } from "../lib/queries/favorites";
import { useUIStore } from "../lib/store";
import { useLayoutContext } from "../components/layout/AppLayout";
import ConfirmDialog from "../components/common/ConfirmDialog";
import GenerationComposer from "../components/generate/GenerationComposer";
import GenerationFeed from "../components/generate/GenerationFeed";
import Lightbox from "../components/lightbox/Lightbox";
import FolderSelector from "../components/favorites/FolderSelector";
import { getImageModelCatalogEntry } from "../lib/modelCatalog";
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
  RetryGenerationRequest,
} from "../types";
import { useTranslation } from "react-i18next";
const DEFAULT_IMAGE_MODEL: ImageModel = "gpt-image-2";
const DEFAULT_IMAGE_MODEL_ENTRY = getImageModelCatalogEntry(DEFAULT_IMAGE_MODEL);

export default function GeneratePage() {
  const { t } = useTranslation();
  const {
    activeProjectId,
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
  const {
    lightbox,
    openLightbox,
    closeLightbox,
    folderSelectorImageId,
    openFolderSelector,
    closeFolderSelector,
  } = useUIStore();
  const [pendingDeleteGenerationId, setPendingDeleteGenerationId] = useState<
    string | null
  >(null);
  const [isDeletingGeneration, setIsDeletingGeneration] = useState(false);
  const [chatViewportHeight, setChatViewportHeight] = useState(0);
  const { data: promptFavorites = [] } = usePromptFavoritesQuery();
  const [promptFavoriteActionKey, setPromptFavoriteActionKey] = useState<
    string | null
  >(null);
  const promptFavoriteCreate = useCreatePromptFavoriteMutation();
  const promptFavoriteDelete = useDeletePromptFavoriteMutation();

  const scrollRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const autoScrollRef = useRef(true);
  const imageModelRef = useRef(imageModel);
  const didUserSelectModelRef = useRef(false);
  const hasComposerDraftRef = useRef(false);
  const activeConversationIdRef = useRef(activeConversationId);

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
    activeConversationIdRef.current = activeConversationId;
  }, [activeConversationId]);

  const promptFavoriteByPrompt = useMemo(() => {
    return new Map(
      promptFavorites.map((favorite) => [
        normalizePromptFavorite(favorite.prompt),
        favorite,
      ]),
    );
  }, [promptFavorites]);

  useEffect(() => {
    if (!activeConversationId) {
      setMessages([]);
      return;
    }
    loadConversationMessages(activeConversationId).catch(() => {});
  }, [activeConversationId, loadConversationMessages]);

  useEffect(() => {
    let cancelled = false;
    const refreshActiveConversation = () => {
      const conversationId = activeConversationIdRef.current;
      if (!conversationId) return;

      loadConversationMessages(conversationId).catch(() => {});
      refreshConversations();
    };

    const completeUnlisten = onGenerationComplete(() => {
      if (!cancelled) {
        refreshActiveConversation();
      }
    });
    const failedUnlisten = onGenerationFailed(() => {
      if (!cancelled) {
        refreshActiveConversation();
      }
    });

    return () => {
      cancelled = true;
      void completeUnlisten.then((unlisten) => unlisten());
      void failedUnlisten.then((unlisten) => unlisten());
    };
  }, [loadConversationMessages, refreshConversations]);

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
          completeGenerationMessage(prev, tempId, result),
        );
        setActiveConversationId(result.conversation_id);
        refreshConversations();
      } catch (e) {
        setMessages((prev) =>
          failGenerationMessage(prev, tempId, e, normalizedRequest),
        );
        refreshConversations();
      }
    },
    [refreshConversations, setActiveConversationId],
  );

  async function handleGenerate() {
    const promptText = prompt.trim();
    if (!promptText) return;

    setPrompt("");
    setEditSources([]);
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
      projectId: activeProjectId,
      editSources: editSources.map((source) => ({ ...source })),
    });
  }

  const handleImageClick = useCallback(
    (images: MessageImage[], index: number) => {
      openLightbox(images, index);
    },
    [openLightbox],
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
        if (
          lightbox &&
          lightbox.images.some(
            (image) => image.generationId === generationId,
          )
        ) {
          closeLightbox();
        }
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
    [promptFavoriteActionKey, promptFavoriteByPrompt, promptFavoriteCreate, promptFavoriteDelete],
  );

  return (
    <div className="flex h-full flex-col">
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto"
      >
        <GenerationFeed
          messages={messages}
          chatViewportHeight={chatViewportHeight}
          isPromptFavorited={(message) =>
            message.role === "user" &&
            promptFavoriteByPrompt.has(normalizePromptFavorite(message.content))
          }
          onImageClick={handleImageClick}
          onDelete={handleRequestDeleteGeneration}
          onEditImage={handleUseImageAsSource}
          onEditPrompt={handleEditPrompt}
          onFavoritePrompt={(value) => void handleTogglePromptFavorite(value)}
          onFavoriteClick={openFolderSelector}
          onRetry={(message) => void handleRetryMessage(message)}
        />
      </div>

      <GenerationComposer
        textareaRef={textareaRef}
        prompt={prompt}
        imageModel={imageModel}
        size={size}
        quality={quality}
        background={background}
        outputFormat={outputFormat}
        moderation={moderation}
        inputFidelity={inputFidelity}
        imageCount={imageCount}
        editSources={editSources}
        editingPromptMessageId={editingPromptMessageId}
        onPromptChange={(value) => {
          markComposerDraftStarted();
          setPrompt(value);
        }}
        onModelChange={handleModelChange}
        onSizeChange={(value) => {
          markComposerDraftStarted();
          setSize(value);
        }}
        onQualityChange={(value) => {
          markComposerDraftStarted();
          setQuality(value);
        }}
        onBackgroundChange={handleBackgroundChange}
        onOutputFormatChange={handleOutputFormatChange}
        onModerationChange={(value) => {
          markComposerDraftStarted();
          setModeration(value);
        }}
        onInputFidelityChange={(value) => {
          markComposerDraftStarted();
          setInputFidelity(value);
        }}
        onImageCountChange={(value) => {
          markComposerDraftStarted();
          setImageCount(value);
        }}
        onAddUploadedSources={() => void handleAddUploadedSources()}
        onClearEditSources={() => {
          markComposerDraftStarted();
          setEditSources([]);
        }}
        onRemoveEditSource={handleRemoveEditSource}
        onCancelPromptEdit={handleCancelPromptEdit}
        onGenerate={() => void handleGenerate()}
      />

      <AnimatePresence>
        {lightbox && (
          <Lightbox
            images={lightbox.images}
            initialIndex={lightbox.index}
            onClose={closeLightbox}
            onEditImage={(image) => {
              handleUseImageAsSource(image);
              closeLightbox();
            }}
            onDelete={handleRequestDeleteGeneration}
          />
        )}
      </AnimatePresence>

      {folderSelectorImageId && (
        <FolderSelector
          imageId={folderSelectorImageId}
          onClose={closeFolderSelector}
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
