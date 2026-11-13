import RecoveryNotice from "../components/generate/RecoveryNotice";
import {
  getGenerationRecipe,
  getGenerationRecipeForRequest,
  recipeToRequest,
  listRecipeProviders,
} from "../lib/recipeApi";
import type { GenerationRecipe } from "../types/recipe";
import {
  useState,
  useCallback,
  useRef,
  useEffect,
  useLayoutEffect,
  useMemo,
} from "react";
import { AnimatePresence } from "framer-motion";
import { useLocation, useNavigate } from "react-router-dom";
import { useModelProviderProfilesQuery } from "../lib/queries/settings";
import { saveProviderSettingsDraft, takeProviderSettingsDraft } from "../lib/providerSettingsDraft";
import {
  editImage,
  generateImage,
  getConversationGenerations,
  getImageModel,
  getModelProviderProfiles,
  saveImageModel,
  deleteGeneration,
  messageImageToEditSource,
  onGenerationComplete,
  onGenerationFailed,
  onGenerationPartial,
  pickSourceImages,
} from "../lib/api";
import { consumePendingEditSources } from "../lib/editSources";
import { consumePendingPrompt } from "../lib/pendingPrompt";
import {
  completeGenerationMessage,
  applyGenerationPartial,
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
import {
  usePromptFavoritesQuery,
  useCreatePromptFavoriteMutation,
  useDeletePromptFavoriteMutation,
} from "../lib/queries/favorites";
import {
  useAcceptPromptAgentDraftMutation,
  useSendPromptAgentMessageMutation,
  useStartPromptAgentSessionMutation,
} from "../lib/queries/promptAgent";
import { capturePreviewSource, useUIStore } from "../lib/store";
import { useLayoutContext } from "../components/layout/AppLayout";
import ConfirmDialog from "../components/common/ConfirmDialog";
import GenerationWorkbench from "../components/generate/GenerationWorkbench";
import GenerationComposer from "../components/generate/GenerationComposer";
import GenerationFeed from "../components/generate/GenerationFeed";
import Lightbox from "../components/lightbox/Lightbox";
import FolderSelector from "../components/favorites/FolderSelector";
import { getImageModelCatalogEntry } from "../lib/modelCatalog";
import type {
  EditSourceImage,
  ComposerMode,
  ImageBackground,
  ImageInputFidelity,
  ImageModeration,
  ImageOutputFormat,
  ImageModel,
  ImageQuality,
  ImageSize,
  Message,
  MessageImage,
  PromptAgentMessage,
  RetryGenerationRequest,
  ImageAdvancedOptions,
} from "../types";
import { image26ValidationError } from "../lib/image26Options";
import { useTranslation } from "react-i18next";
const DEFAULT_IMAGE_MODEL: ImageModel = "gpt-image-2.6-flare";
const DEFAULT_IMAGE_MODEL_ENTRY =
  getImageModelCatalogEntry(DEFAULT_IMAGE_MODEL);

export default function GeneratePage() {
  const { t } = useTranslation();
  const location = useLocation();
  const navigate = useNavigate();
  const draftRevision = useRef(0);
  const submissionPending = useRef(false);
  const unknownOutcomes = useRef(new Set<string>());
  const [pendingUnknownRetry, setPendingUnknownRetry] = useState<RetryGenerationRequest|null>(null);
  const restoringSettingsDraft = useRef(false);
  const restoredIntentState = useRef<unknown>(Symbol("no restored intent"));
  const [providerSetup, setProviderSetup] = useState<{model:ImageModel;providerId?:string}|null>(null);
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
  const [advancedOptions, setAdvancedOptions] = useState<ImageAdvancedOptions>(
    {},
  );
  const [outputCompression, setOutputCompression] = useState(100);
  const [aspectRatio, setAspectRatio] = useState<string | undefined>();
  const [resolution, setResolution] = useState<string | undefined>();
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
  const [inputFidelity, setInputFidelity] = useState<ImageInputFidelity>(
    DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.inputFidelity,
  );
  const [imageCount, setImageCount] = useState(
    DEFAULT_IMAGE_MODEL_ENTRY.parameterDefaults.imageCount,
  );
  const [imageModel, setImageModel] = useState<ImageModel>(DEFAULT_IMAGE_MODEL);
  const providerQuery = useModelProviderProfilesQuery(imageModel);
  useEffect(() => { void providerQuery.refetch(); }, [imageModel]);
  const [editSources, setEditSources] = useState<EditSourceImage[]>([]);
  const hydratedSourcePath = useRef<{ path: string | undefined } | null>(null);
  const [recipeIdentity, setRecipeIdentity] = useState<{
    providerProfileId?: string;
    providerOverrideConfirmed?: boolean;
    sourceGenerationId?: string;
  }>({});
  const [recipeWarnings, setRecipeWarnings] = useState<string[]>([]);
  useEffect(() => {
    if (!recipeIdentity.providerProfileId) return;
    const exact = providerQuery.data?.profiles.find(profile => profile.id === recipeIdentity.providerProfileId);
    if (exact?.api_key.trim()) setRecipeWarnings(current => current.includes("provider_missing") ? current.filter(warning => warning !== "provider_missing") : current);
  }, [providerQuery.data, recipeIdentity.providerProfileId]);
  const [recipeProviders, setRecipeProviders] = useState<
    { id: string; name: string }[]
  >([]);
  const [chosenRecipeProvider, setChosenRecipeProvider] = useState("");
  const providerNeedsReview = recipeWarnings.some((w) =>
    [
      "legacy_incomplete",
      "provider_missing",
      "endpoint_changed",
      "provider_unconfigured",
    ].includes(w),
  );
  useEffect(() => {
    if (!providerNeedsReview) return;
    let cancelled = false;
    setRecipeProviders([]);
    setChosenRecipeProvider("");
    listRecipeProviders(imageModel)
      .then((providers) => {
        if (!cancelled) {
          setRecipeProviders(providers);
          setChosenRecipeProvider(providers[0]?.id ?? "");
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [providerNeedsReview, imageModel]);
  const [pendingRecipe, setPendingRecipe] = useState<GenerationRecipe | null>(
    null,
  );
  const [recipeError, setRecipeError] = useState<string | null>(null);
  const recipeLoadToken = useRef(0);
  const recipeLoadPending = useRef(false);
  const firstSourcePath = editSources[0]?.path;
  useEffect(() => {
    if (restoringSettingsDraft.current) {
      if (hydratedSourcePath.current?.path === firstSourcePath) restoringSettingsDraft.current = false;
      return;
    }
    if (hydratedSourcePath.current?.path === firstSourcePath) {
      hydratedSourcePath.current = null;
      return;
    }
    hydratedSourcePath.current = null;
    setAdvancedOptions((current) => {
      if (!current.markupPath && !current.maskPath) return current;
      return { ...current, markupPath: undefined, maskPath: undefined };
    });
  }, [firstSourcePath]);
  const [composerMode, setComposerMode] = useState<ComposerMode>("direct");
  const [promptAgentSessionId, setPromptAgentSessionId] = useState<
    string | null
  >(null);
  const [promptAgentMessages, setPromptAgentMessages] = useState<
    PromptAgentMessage[]
  >([]);
  const [selectedPromptAgentConfigId, setSelectedPromptAgentConfigId] =
    useState("");
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
  const startPromptAgentSessionMutation = useStartPromptAgentSessionMutation();
  const sendPromptAgentMessageMutation = useSendPromptAgentMessageMutation();
  const acceptPromptAgentDraftMutation = useAcceptPromptAgentDraftMutation();

  const scrollRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const autoScrollRef = useRef(true);
  const imageModelRef = useRef(imageModel);
  const didUserSelectModelRef = useRef(false);
  const hasComposerDraftRef = useRef(false);
  const activeConversationIdRef = useRef(activeConversationId);
  const conversationLoadRequestIdRef = useRef(0);
  const messagesVersionRef = useRef(0);
  const navigationPendingPrompt = useMemo(() => {
    const state = location.state as { pendingPrompt?: string } | null;
    return typeof state?.pendingPrompt === "string"
      ? state.pendingPrompt
      : null;
  }, [location.state]);

  const setActiveImageModel = useCallback((model: ImageModel) => {
    imageModelRef.current = model;
    setImageModel(model);
  }, []);

  const markComposerDraftStarted = useCallback(() => {
    draftRevision.current++;
    hasComposerDraftRef.current = true;
  }, []);

  useLayoutEffect(() => {
    const saved = takeProviderSettingsDraft();
    if (!saved) return;
    const request = saved.request;
    // /generate has no conversation segment. Restore its routing context as
    // well as the composer, without replaying a prior recipe launch intent.
    if (location.pathname === "/generate" && request.conversationId) {
      navigate("/generate", {
        replace: true,
        state: { activateConversationId: request.conversationId },
      });
    }
    restoredIntentState.current = location.state;
    navigationRecipeHandled.current = location.state;
    didUserSelectModelRef.current = true; hasComposerDraftRef.current = true;
    restoringSettingsDraft.current = true; hydratedSourcePath.current = {path: request.editSources[0]?.path};
    setActiveImageModel(request.model); setPrompt(request.prompt); setSize(request.size);
    setQuality(request.quality); setBackground(request.background); setOutputFormat(request.outputFormat);
    setModeration(request.moderation); setInputFidelity(request.inputFidelity); setImageCount(request.imageCount);
    setAspectRatio(request.aspectRatio); setResolution(request.resolution); setAdvancedOptions(request.advancedOptions ?? {});
    setOutputCompression(request.outputCompression ?? 100); setEditSources(request.editSources);
    setRecipeIdentity({providerProfileId:request.providerProfileId,providerOverrideConfirmed:request.providerOverrideConfirmed,sourceGenerationId:request.sourceGenerationId});
    setRecipeWarnings(saved.recipeWarnings ?? []); setComposerMode(saved.composerMode ?? "direct");
    setEditingPromptMessageId(saved.editingPromptMessageId);setPromptAgentSessionId(saved.promptAgentSessionId);
    setPromptAgentMessages(saved.promptAgentMessages ?? []);setSelectedPromptAgentConfigId(saved.selectedPromptAgentConfigId ?? "");
  }, [setActiveImageModel]);

  const applyModelDefaults = useCallback((model: ImageModel) => {
    const { parameterDefaults, supportsEdit } =
      getImageModelCatalogEntry(model);

    setAdvancedOptions({});
    setOutputCompression(100);
    setAspectRatio(parameterDefaults.aspectRatio);
    setResolution(parameterDefaults.resolution);
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
    const { parameterDefaults, parameterCapabilities, supportsEdit } =
      getImageModelCatalogEntry(model);

    setAspectRatio((current) =>
      parameterCapabilities.aspectRatios?.includes(current ?? "")
        ? current
        : parameterDefaults.aspectRatio,
    );
    setResolution((current) =>
      parameterCapabilities.resolutions?.includes(current ?? "")
        ? current
        : parameterDefaults.resolution,
    );
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
      supportsEdit &&
      parameterCapabilities.inputFidelityOptions.includes(current)
        ? current
        : parameterDefaults.inputFidelity,
    );

    if (!supportsEdit) {
      setEditSources([]);
    }
  }, []);

  const loadConversationMessages = useCallback(
    async (conversationId: string) => {
      const requestId = ++conversationLoadRequestIdRef.current;
      const messagesVersionAtStart = messagesVersionRef.current;
      const generations = await getConversationGenerations(conversationId);
      if (
        requestId !== conversationLoadRequestIdRef.current ||
        activeConversationIdRef.current !== conversationId ||
        messagesVersionRef.current !== messagesVersionAtStart
      ) {
        return;
      }
      messagesVersionRef.current += 1;
      setMessages(generationsToMessages(generations));
    },
    [],
  );

  useEffect(() => {
    activeConversationIdRef.current = activeConversationId;
  }, [activeConversationId]);

  const isGenerating = useMemo(
    () => messages.some((m) => m.status === "processing"),
    [messages],
  );

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
      conversationLoadRequestIdRef.current += 1;
      messagesVersionRef.current += 1;
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

    const partialUnlisten = onGenerationPartial((event) => {
      if (!cancelled) {
        setMessages((current) => applyGenerationPartial(current, event));
      }
    });
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
      void partialUnlisten.then((unlisten) => unlisten());
      void completeUnlisten.then((unlisten) => unlisten());
      void failedUnlisten.then((unlisten) => unlisten());
    };
  }, [loadConversationMessages, refreshConversations]);

  useLayoutEffect(() => {
    if (autoScrollRef.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, promptAgentMessages, chatViewportHeight]);

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
      if (autoScrollRef.current)
        scrollElement.scrollTop = scrollElement.scrollHeight;
    });
    resizeObserver.observe(scrollElement);
    if (scrollElement.firstElementChild)
      resizeObserver.observe(scrollElement.firstElementChild);

    return () => {
      resizeObserver.disconnect();
    };
  }, [messages.length, promptAgentMessages.length]);

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
    if (restoredIntentState.current === location.state) return;
    const pendingSources = consumePendingEditSources();
    if (pendingSources.length > 0) {
      markComposerDraftStarted();
      setEditSources((current) => mergeEditSources(current, pendingSources));
    }
  }, [markComposerDraftStarted]);

  useEffect(() => {
    if (restoredIntentState.current === location.state) return;
    if (navigationPendingPrompt) {
      markComposerDraftStarted();
      setPrompt(navigationPendingPrompt);
      return;
    }

    const pendingPrompt = consumePendingPrompt();
    if (!pendingPrompt) return;

    markComposerDraftStarted();
    setPrompt(pendingPrompt);
  }, [markComposerDraftStarted, navigationPendingPrompt]);

  useEffect(() => {
    let cancelled = false;

    getImageModel()
      .then((model) => {
        if (cancelled || didUserSelectModelRef.current) {
          return;
        }

        setActiveImageModel(model);

        if (!hasComposerDraftRef.current) {
          applyModelDefaults(model);
        } else {
          reconcileDraftStateForModel(model);
        }
      })
      .catch(() => {});

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
      if (
        value === "transparent" &&
        !["png", "webp", "svg"].includes(outputFormat)
      ) {
        setOutputFormat("png");
      }
    },
    [markComposerDraftStarted, outputFormat],
  );

  const handleOutputFormatChange = useCallback(
    (value: ImageOutputFormat) => {
      markComposerDraftStarted();
      setOutputFormat(value);
      if (value === "svg") setImageCount(1);
      if (value === "gif") {
        setSize("auto");
        setAdvancedOptions((current) => ({ ...current, maskPath: undefined }));
      }
      if (
        !["png", "webp", "svg"].includes(value) &&
        background === "transparent"
      ) {
        setBackground("auto");
      }
    },
    [background, markComposerDraftStarted],
  );

  const handleUseImageAsSource = useCallback(
    (image: MessageImage) => {
      markComposerDraftStarted();
      if (!modelSupportsEdit(imageModelRef.current)) {
        textareaRef.current?.focus();
        return;
      }
      setEditSources((current) =>
        mergeEditSources(current, [messageImageToEditSource(image)]),
      );
      textareaRef.current?.focus();
    },
    [markComposerDraftStarted],
  );

  const handleModelChange = useCallback(
    (model: ImageModel) => {
      didUserSelectModelRef.current = true;
      markComposerDraftStarted();
      setProviderSetup(null);
      setActiveImageModel(model);
      applyModelDefaults(model);
      setRecipeIdentity({});
      setRecipeWarnings([]);
      saveImageModel(model).catch(() => {});
    },
    [applyModelDefaults, markComposerDraftStarted, setActiveImageModel],
  );

  const handleRemoveEditSource = useCallback(
    (sourceId: string) => {
      markComposerDraftStarted();
      setEditSources((current) =>
        current.filter((source) => source.id !== sourceId),
      );
    },
    [markComposerDraftStarted],
  );

  const submitGenerationRequest = useCallback(
    async (request: RetryGenerationRequest) => {
      const promptText = request.prompt.trim();
      if (!promptText || submissionPending.current) return false;
      submissionPending.current = true;
      try {
        const saved = await getModelProviderProfiles(request.model);
        const profile = saved.profiles.find(profile => profile.id === (request.providerProfileId ?? saved.active_provider_id));
        if (!profile?.api_key.trim()) {
          setProviderSetup({model:request.model,providerId:request.providerProfileId});
          if (request.providerProfileId && !profile) setRecipeWarnings(current => [...new Set([...current, "provider_missing"])]);
          submissionPending.current = false;
          return false;
        }
        setProviderSetup(null);
      } catch {
        setProviderSetup({model:request.model,providerId:request.providerProfileId});
        submissionPending.current = false;
        return false;
      }

      const tempId = crypto.randomUUID();
      const retryRequest: RetryGenerationRequest = {
        ...request,
        prompt: promptText,
        editSources: request.editSources.map((source) => ({ ...source })),
        clientRequestId: tempId,
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
      messagesVersionRef.current += 1;
      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      autoScrollRef.current = true;

      try {
        const invalid = image26ValidationError({
          model: normalizedRequest.model,
          format: normalizedRequest.outputFormat,
          size: normalizedRequest.size,
          count: normalizedRequest.imageCount,
          background: normalizedRequest.background,
          editing: normalizedRequest.editSources.length > 0,
          options: normalizedRequest.advancedOptions ?? {},
          compression: normalizedRequest.outputCompression,
        });
        if (invalid) throw new Error(t(`generate.advanced.${invalid}`));
        const maxSources =
          getImageModelCatalogEntry(normalizedRequest.model)
            .maxReferenceImages ?? 16;
        if (normalizedRequest.editSources.length > maxSources) {
          throw new Error(
            t("generate.tooManyReferences", { count: maxSources }),
          );
        }
        const result =
          normalizedRequest.editSources.length > 0
            ? await editImage(buildEditParams(normalizedRequest))
            : await generateImage(buildGenerationParams(normalizedRequest));
        messagesVersionRef.current += 1;
        setMessages((prev) => completeGenerationMessage(prev, tempId, result));
        setActiveConversationId(result.conversation_id);
        refreshConversations();
        return true;
      } catch (e) {
        const failureText = e instanceof Error ? e.message : typeof e === "string" ? e : JSON.stringify(e);
        if ((e as {kind?:string})?.kind === "Network" || /unknown|outcome|timeout|timed out|connection lost|connection reset/i.test(failureText ?? "")) unknownOutcomes.current.add(tempId);
        if (["ApiKeyNotSet", "ProviderProfileNotFound"].includes((e as {kind?:string})?.kind ?? "")) setProviderSetup({model:request.model,providerId:request.providerProfileId});
        messagesVersionRef.current += 1;
        setMessages((prev) =>
          failGenerationMessage(
            prev,
            tempId,
            (e as { kind?: string })?.kind === "Validation" &&
              (e as { detail?: { message?: string } })?.detail?.message ===
                "Unsupported image parameters for the selected model."
              ? t("generate.unsupportedParameters")
              : e,
            normalizedRequest,
          ),
        );
        refreshConversations();
        return false;
      } finally { submissionPending.current = false; }
    },
    [refreshConversations, setActiveConversationId, t],
  );

  const hydrateRecipe = useCallback(
    (recipe: GenerationRecipe) => {
      const request = recipeToRequest(recipe);
      didUserSelectModelRef.current = true;
      markComposerDraftStarted();
      hydratedSourcePath.current = { path: request.editSources[0]?.path };
      setActiveImageModel(request.model);
      setPrompt(request.prompt);
      setSize(request.size);
      setQuality(request.quality);
      setBackground(request.background);
      setOutputFormat(request.outputFormat);
      setModeration(request.moderation);
      setInputFidelity(request.inputFidelity);
      setImageCount(request.imageCount);
      setAspectRatio(request.aspectRatio);
      setResolution(request.resolution);
      setAdvancedOptions(request.advancedOptions ?? {});
      setOutputCompression(request.outputCompression ?? 100);
      setEditSources(request.editSources);
      setRecipeIdentity({
        providerProfileId: request.providerProfileId,
        sourceGenerationId: request.sourceGenerationId,
      });
      setRecipeWarnings(recipe.warnings);
      setComposerMode("direct");
      setEditingPromptMessageId(null);
      setPendingRecipe(null);
      textareaRef.current?.focus();
    },
    [setActiveImageModel, markComposerDraftStarted],
  );

  const handleReuseRecipe = useCallback(
    async (generationId: string, mode: "edit" | "generate", outcomeUnknown = false) => {
      if (recipeLoadPending.current) return;
      recipeLoadPending.current = true;
      const token = ++recipeLoadToken.current;
      setRecipeError(null);
      try {
        const loaded = await getGenerationRecipe(generationId);
        const recipe = outcomeUnknown && !loaded.warnings.includes("provider_outcome_unknown")
          ? { ...loaded, warnings: [...loaded.warnings, "provider_outcome_unknown"] }
          : loaded;
        if (token !== recipeLoadToken.current) return;
        if (mode === "generate" && recipe.warnings.length === 0) {
          await submitGenerationRequest({
            ...recipeToRequest(recipe),
            conversationId: activeConversationId,
            projectId: activeProjectId,
          });
        } else if (hasComposerDraftRef.current || recipe.warnings.length > 0) {
          setPendingRecipe(recipe);
        } else {
          hydrateRecipe(recipe);
        }
      } catch {
        if (token === recipeLoadToken.current)
          setRecipeError(t("recipe.loadFailed"));
      } finally {
        recipeLoadPending.current = false;
      }
    },
    [
      activeConversationId,
      activeProjectId,
      hydrateRecipe,
      submitGenerationRequest,
      t,
    ],
  );
  const navigationRecipeHandled = useRef<unknown>(null);
  useEffect(() => {
    const state = location.state as {
      recipeGenerationId?: string;
      recipeMode?: "edit" | "generate";
    } | null;
    if (
      state?.recipeGenerationId &&
      navigationRecipeHandled.current !== state
    ) {
      navigationRecipeHandled.current = state;
      void handleReuseRecipe(
        state.recipeGenerationId,
        state.recipeMode ?? "edit",
      );
    }
  }, [location.state, handleReuseRecipe]);

  async function handleGenerate() {
    const promptText = prompt.trim();
    if (!promptText || recipeWarnings.length > 0) return;

    const revision = draftRevision.current;
    const submitted = await submitGenerationRequest({
      ...recipeIdentity,
      prompt: promptText,
      advancedOptions,
      outputCompression,
      model: imageModel,
      size,
      aspectRatio,
      resolution,
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
    if (submitted && revision === draftRevision.current) {
      setPrompt(""); hasComposerDraftRef.current = false; setEditSources([]); setEditingPromptMessageId(null);
    }
  }

  const handleDeepThinkingSend = useCallback(async () => {
    const trimmed = prompt.trim();
    if (!trimmed || isGenerating) return;

    if (!selectedPromptAgentConfigId) {
      setPromptAgentMessages((current) => [
        ...current,
        {
          id: `agent-error-${Date.now()}`,
          session_id: promptAgentSessionId ?? "local",
          role: "assistant",
          content: t("generate.agent.selectLlm"),
          draft_prompt: null,
          selected_skill_ids: [],
          suggested_params: {},
          ready_to_generate: false,
          created_at: new Date().toISOString(),
        },
      ]);
      return;
    }

    const sourceImagePaths = editSources.map((source) => source.path);
    const result = promptAgentSessionId
      ? await sendPromptAgentMessageMutation.mutateAsync({
          sessionId: promptAgentSessionId,
          message: trimmed,
          configId: selectedPromptAgentConfigId,
          sourceImagePaths,
        })
      : await startPromptAgentSessionMutation.mutateAsync({
          prompt: trimmed,
          configId: selectedPromptAgentConfigId,
          conversationId: activeConversationId,
          projectId: activeProjectId,
          sourceImagePaths,
        });

    setPromptAgentSessionId(result.session.id);
    setPromptAgentMessages(result.messages);
    setPrompt("");
  }, [
    activeConversationId,
    activeProjectId,
    editSources,
    isGenerating,
    prompt,
    promptAgentSessionId,
    selectedPromptAgentConfigId,
    sendPromptAgentMessageMutation,
    startPromptAgentSessionMutation,
    t,
  ]);

  const handleAcceptAgentDraft = useCallback(
    async (message: PromptAgentMessage) => {
      const draft = message.draft_prompt?.trim();
      if (!draft || !promptAgentSessionId || recipeWarnings.length > 0) return;

      const accepted = await acceptPromptAgentDraftMutation.mutateAsync({
        sessionId: promptAgentSessionId,
        acceptedPrompt: draft,
      });
      const acceptedPrompt = accepted.accepted_prompt ?? draft;
      const nextModel = message.suggested_params.model ?? imageModel;
      const nextSize = message.suggested_params.size ?? size;
      const nextQuality = message.suggested_params.quality ?? quality;
      const nextBackground = message.suggested_params.background ?? background;
      const nextOutputFormat =
        message.suggested_params.output_format ?? outputFormat;
      const nextModeration = message.suggested_params.moderation ?? moderation;
      const nextImageCount = message.suggested_params.image_count ?? imageCount;

      setPrompt(acceptedPrompt);
      setActiveImageModel(nextModel);
      setSize(nextSize);
      setQuality(nextQuality);
      setBackground(nextBackground);
      setOutputFormat(nextOutputFormat);
      setModeration(nextModeration);
      setImageCount(nextImageCount);
      setComposerMode("direct");
      setPromptAgentMessages([]);
      setPromptAgentSessionId(null);

      await submitGenerationRequest({
        ...recipeIdentity,
        prompt: acceptedPrompt,
        advancedOptions,
        outputCompression,
        model: nextModel,
        size: nextSize,
        aspectRatio:
          nextModel === imageModel
            ? aspectRatio
            : getImageModelCatalogEntry(nextModel).parameterDefaults
                .aspectRatio,
        resolution:
          nextModel === imageModel
            ? resolution
            : getImageModelCatalogEntry(nextModel).parameterDefaults.resolution,
        quality: nextQuality,
        background: nextBackground,
        outputFormat: nextOutputFormat,
        moderation: nextModeration,
        inputFidelity,
        imageCount: nextImageCount,
        conversationId: activeConversationId,
        projectId: activeProjectId,
        editSources: editSources.map((source) => ({ ...source })),
      });
    },
    [
      advancedOptions,
      outputCompression,
      aspectRatio,
      resolution,
      acceptPromptAgentDraftMutation,
      recipeIdentity,
      recipeWarnings,
      activeConversationId,
      activeProjectId,
      background,
      editSources,
      imageCount,
      imageModel,
      inputFidelity,
      moderation,
      outputFormat,
      promptAgentSessionId,
      quality,
      setActiveImageModel,
      size,
      submitGenerationRequest,
    ],
  );

  const handleContinueAgentDraft = useCallback(
    (message: PromptAgentMessage) => {
      setPrompt(message.draft_prompt ?? "");
      textareaRef.current?.focus();
    },
    [],
  );

  const handleEditAgentDraft = useCallback((message: PromptAgentMessage) => {
    setPrompt(message.draft_prompt ?? "");
    setComposerMode("direct");
    textareaRef.current?.focus();
  }, []);

  const handleImageClick = useCallback(
    (images: MessageImage[], index: number) => {
      const image = images[index];
      const origin =
        image &&
        [
          ...document.querySelectorAll<HTMLElement>("[data-preview-image-id]"),
        ].find((node) => node.dataset.previewImageId === image.imageId);
      openLightbox(
        images,
        index,
        origin ? capturePreviewSource(origin, image.imageId) : undefined,
      );
    },
    [openLightbox],
  );

  const handleDeleteFromBubble = useCallback(
    async (generationId: string) => {
      setIsDeletingGeneration(true);
      try {
        await deleteGeneration(generationId);
        messagesVersionRef.current += 1;
        setMessages((prev) =>
          prev.filter(
            (m) =>
              m.generationId !== generationId &&
              m.id !== `user-${generationId}`,
          ),
        );
        if (
          lightbox &&
          lightbox.images.some((image) => image.generationId === generationId)
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
      const clientRequestId = message.retryRequest?.clientRequestId;
      if (clientRequestId && message.id === `assistant-${clientRequestId}`) {
        try {
          const saved = await getGenerationRecipeForRequest(clientRequestId);
          if (saved) {
            await handleReuseRecipe(saved.generation_id, "generate", unknownOutcomes.current.has(clientRequestId));
            return;
          }
        } catch {
          setRecipeError(t("recipe.loadFailed"));
          return;
        }
      }
      if (message.retryRequest && unknownOutcomes.current.has(message.retryRequest.clientRequestId ?? "")) {
        setPendingUnknownRetry(message.retryRequest);
        return;
      }
      if (
        message.generationId &&
        !message.id.startsWith(
          "assistant-" + message.retryRequest?.clientRequestId,
        )
      ) {
        await handleReuseRecipe(message.generationId, "generate");
        return;
      }
      if (!message.retryRequest) return;
      await submitGenerationRequest(message.retryRequest);
    },
    [submitGenerationRequest, handleReuseRecipe, t],
  );

  const handleEditPrompt = useCallback(
    (message: Message) => {
      markComposerDraftStarted();
      setPrompt(message.content);
      setEditSources(() =>
        modelSupportsEdit(imageModelRef.current)
          ? (message.sourceImages?.map((image) =>
              messageImageToEditSource(image),
            ) ?? [])
          : [],
      );
      setEditingPromptMessageId(message.id);
      autoScrollRef.current = false;
      textareaRef.current?.focus();
    },
    [markComposerDraftStarted],
  );

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
    [
      promptFavoriteActionKey,
      promptFavoriteByPrompt,
      promptFavoriteCreate,
      promptFavoriteDelete,
    ],
  );

  const configuredProfile = providerQuery.data?.profiles.find(profile => profile.id === (recipeIdentity.providerProfileId ?? providerQuery.data?.active_provider_id));
  const needsProviderSetup = !!providerSetup || (!!providerQuery.data && !configuredProfile?.api_key.trim()) || providerQuery.isError;
  function openProviderSettings() {
    try {
      saveProviderSettingsDraft({request:{...recipeIdentity,prompt,advancedOptions,outputCompression,model:imageModel,size,aspectRatio,resolution,quality,background,outputFormat,moderation,inputFidelity,imageCount,conversationId:activeConversationId,projectId:activeProjectId,editSources},recipeWarnings,composerMode,editingPromptMessageId,promptAgentSessionId,promptAgentMessages,selectedPromptAgentConfigId});
    } catch { setRecipeError(t("generate.draftSaveFailed")); return; }
    const target=providerSetup ?? {model:imageModel,providerId:recipeIdentity.providerProfileId};
    const query=new URLSearchParams({tab:"model",model:target.model,returnTo:location.pathname || "/generate"});
    if(target.providerId)query.set("provider",target.providerId);
    navigate(`/settings?${query.toString()}`);
  }

  return (
    <GenerationWorkbench>
      {(parametersTarget, drawerOpen) => (
        <>
          <RecoveryNotice />
          <ConfirmDialog open={pendingUnknownRetry !== null} title={t("generate.unknownRetryTitle")} description={t("generate.unknownRetryDescription")} confirmLabel={t("generate.confirmUnknownRetry")} cancelLabel={t("favorites.cancel")} onCancel={() => setPendingUnknownRetry(null)} onConfirm={() => { const request = pendingUnknownRetry; setPendingUnknownRetry(null); if (request) void submitGenerationRequest(request); }} />
          <div
            ref={scrollRef}
            onScroll={handleScroll}
            className="flex-1 min-h-0 overflow-y-auto"
          >
            <GenerationFeed
              messages={messages}
              chatViewportHeight={chatViewportHeight}
              isPromptFavorited={(message) =>
                message.role === "user" &&
                promptFavoriteByPrompt.has(
                  normalizePromptFavorite(message.content),
                )
              }
              onImageClick={handleImageClick}
              onDelete={handleRequestDeleteGeneration}
              onEditImage={handleUseImageAsSource}
              onEditPrompt={handleEditPrompt}
              onFavoritePrompt={(value) =>
                void handleTogglePromptFavorite(value)
              }
              onFavoriteClick={openFolderSelector}
              onRetry={(message) => void handleRetryMessage(message)}
              onReuseRecipe={(id, mode) => void handleReuseRecipe(id, mode)}
              agentMessages={promptAgentMessages}
              isPromptAgentRunning={
                startPromptAgentSessionMutation.isPending ||
                sendPromptAgentMessageMutation.isPending
              }
              onAcceptAgentDraft={(message) =>
                void handleAcceptAgentDraft(message)
              }
              onContinueAgentDraft={handleContinueAgentDraft}
              onEditAgentDraft={handleEditAgentDraft}
            />
          </div>

          {recipeError && (
            <p role="alert" className="px-5 py-2 text-sm text-error">
              {recipeError}
            </p>
          )}
          {recipeWarnings.length > 0 && (
            <div
              role="status"
              className="border-t border-border-subtle bg-surface px-5 py-3 text-sm"
            >
              <p>{t("recipe.reviewRequired")}</p>
              <ul className="my-2 list-inside list-disc text-muted">
                {recipeWarnings.map((w) => (
                  <li key={w}>{t(`recipe.warnings.${w}`)}</li>
                ))}
              </ul>
              <div className="flex flex-wrap gap-2">
                {providerNeedsReview && (
                  <div className="grid w-full gap-2 sm:flex sm:items-end">
                    <label className="grid gap-1 text-xs">
                      {t("recipe.providerLabel")}
                      <select
                        className="studio-control focus-ring min-h-9 rounded-lg px-3"
                        value={chosenRecipeProvider}
                        onChange={(event) =>
                          setChosenRecipeProvider(event.target.value)
                        }
                      >
                        <option value="" disabled>
                          {t("recipe.providerLabel")}
                        </option>
                        {recipeProviders.map((provider) => (
                          <option key={provider.id} value={provider.id}>
                            {provider.name}
                          </option>
                        ))}
                      </select>
                    </label>
                    <button
                      type="button"
                      disabled={!chosenRecipeProvider}
                      className="studio-control focus-ring min-h-9 rounded-lg px-3 disabled:opacity-50"
                      onClick={() => {
                        setProviderSetup(null);
                        setRecipeIdentity((current) => ({
                          ...current,
                          providerProfileId: chosenRecipeProvider,
                          providerOverrideConfirmed: true,
                        }));
                        setRecipeWarnings((current) =>
                          current.filter(
                            (w) =>
                              ![
                                "legacy_incomplete",
                                "provider_missing",
                                "endpoint_changed",
                                "provider_unconfigured",
                              ].includes(w),
                          ),
                        );
                      }}
                    >
                      {t("recipe.confirmProvider")}
                    </button>
                    {recipeProviders.length === 0 && (
                      <p className="text-xs text-muted">
                        {t("recipe.noProviders")}
                      </p>
                    )}
                  </div>
                )}

                {recipeWarnings.some((w) =>
                  [
                    "source_missing",
                    "sources_require_selection",
                    "invalid_job",
                  ].includes(w),
                ) && (
                  <button
                    type="button"
                    className="studio-control focus-ring min-h-9 rounded-lg px-3"
                    onClick={() => {
                      setEditSources([]);
                      setAdvancedOptions((current) => ({
                        ...current,
                        markupPath: undefined,
                        maskPath: undefined,
                      }));
                      setRecipeIdentity((current) => ({
                        ...current,
                        sourceGenerationId: undefined,
                      }));
                      setRecipeWarnings((current) =>
                        current.filter(
                          (w) =>
                            ![
                              "source_missing",
                              "sources_require_selection",
                              "invalid_job",
                            ].includes(w),
                        ),
                      );
                    }}
                  >
                    {t("recipe.removeSources")}
                  </button>
                )}
                {recipeWarnings.includes("provider_outcome_unknown") && (
                  <button
                    type="button"
                    className="studio-control focus-ring min-h-9 rounded-lg px-3"
                    onClick={() =>
                      setRecipeWarnings((current) =>
                        current.filter((w) => w !== "provider_outcome_unknown"),
                      )
                    }
                  >
                    {t("recipe.acknowledgeNewCharge")}
                  </button>
                )}
                {recipeWarnings.includes("invalid_parameters") && (
                  <button
                    type="button"
                    className="studio-control focus-ring min-h-9 rounded-lg px-3"
                    onClick={() => {
                      applyModelDefaults(imageModel);

                      setRecipeWarnings((current) =>
                        current.filter((w) => w !== "invalid_parameters"),
                      );
                    }}
                  >
                    {t("recipe.resetParameters")}
                  </button>
                )}
              </div>
            </div>
          )}
          {needsProviderSetup && <div role="status" className="flex flex-wrap items-center justify-between gap-2 border-t border-border-subtle px-5 py-3 text-sm"><span className="text-muted">{t("generate.providerSetupRequired")}</span><button type="button" onClick={openProviderSettings} className="studio-control focus-ring min-h-9 rounded-lg px-3 py-2 text-xs font-medium">{t("generate.configureProvider")}</button></div>}
          <GenerationComposer
            parametersTarget={parametersTarget}
            interactionDisabled={drawerOpen}
            advancedOptions={advancedOptions}
            onAdvancedOptionsChange={(value) => {
              markComposerDraftStarted();
              setAdvancedOptions(value);
            }}
            outputCompression={outputCompression}
            onOutputCompressionChange={(value) => {
              markComposerDraftStarted();
              setOutputCompression(value);
            }}
            textareaRef={textareaRef}
            prompt={prompt}
            imageModel={imageModel}
            size={size}
            aspectRatio={aspectRatio}
            resolution={resolution}
            onSizeChange={(value) => {
              markComposerDraftStarted();
              setSize(value);
            }}
            onAspectRatioChange={(value) => {
              markComposerDraftStarted();
              setAspectRatio(value);
            }}
            onResolutionChange={(value) => {
              markComposerDraftStarted();
              setResolution(value);
            }}
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
            composerMode={composerMode}
            onComposerModeChange={setComposerMode}
            onDeepThinkingSend={() => void handleDeepThinkingSend()}
            onPromptAgentConfigChange={setSelectedPromptAgentConfigId}
            isPromptAgentRunning={
              startPromptAgentSessionMutation.isPending ||
              sendPromptAgentMessageMutation.isPending
            }
            isGenerating={isGenerating || recipeWarnings.length > 0}
          />

          <AnimatePresence>
            {lightbox && (
              <Lightbox
                images={lightbox.images}
                source={lightbox.source}
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
            open={pendingRecipe !== null}
            title={t("recipe.replaceDraft")}
            description={
              pendingRecipe?.warnings.length
                ? pendingRecipe.warnings
                    .map((w) => t(`recipe.warnings.${w}`))
                    .join(" · ")
                : t("recipe.replaceDraftDescription")
            }
            confirmLabel={t(
              pendingRecipe?.warnings.includes("unknown_model")
                ? "recipe.useCurrentModel"
                : "recipe.reviewParameters",
            )}
            cancelLabel={t("favorites.cancel")}
            onConfirm={() => {
              if (pendingRecipe) {
                try {
                  hydrateRecipe(
                    pendingRecipe.warnings.includes("unknown_model")
                      ? {
                          ...pendingRecipe,
                          model: imageModel,
                          options: {},
                          provider_profile_id: undefined,
                          warnings: pendingRecipe.warnings.filter(
                            (w) => w !== "unknown_model",
                          ),
                        }
                      : pendingRecipe,
                  );
                } catch {
                  setRecipeError(t("recipe.loadFailed"));
                  setPendingRecipe(null);
                }
              }
            }}
            onCancel={() => setPendingRecipe(null)}
          />
          <ConfirmDialog
            open={pendingDeleteGenerationId !== null}
            title={t("lightbox.deleteConfirm")}
            confirmLabel={t("favorites.confirm")}
            cancelLabel={t("favorites.cancel")}
            onConfirm={() => void handleConfirmDeleteGeneration()}
            onCancel={handleCancelDeleteGeneration}
            loading={isDeletingGeneration}
          />
        </>
      )}
    </GenerationWorkbench>
  );
}
