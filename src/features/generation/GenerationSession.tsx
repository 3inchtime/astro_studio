import { useChatViewport } from "./useChatViewport";
import { usePromptFavorites } from "./usePromptFavorites";
import { consumeNavigationIntent } from "./navigationIntent";
import { ROUTES } from "../../app/routes/contracts";
import { applyAgentSuggestions, defaultParameters, parametersFromRequest, reconcileParameters, type ComposerParameters } from "./composerParameters";
import { useConversationFeed } from "./useConversationFeed";
import { resolveVisualSourcePaths, promptVisionErrorKey } from "../../lib/promptVision";
import RecoveryNotice from "../../components/generate/RecoveryNotice";
import {
  getGenerationRecipe,
  getGenerationRecipeForRequest,
  recipeToRequest,
  listRecipeProviders,
} from "../../lib/recipeApi";
import type { GenerationRecipe } from "../../types/recipe";
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
import { useModelProviderProfilesQuery } from "../../lib/queries/settings";
import { saveProviderSettingsDraft, takeProviderSettingsDraft, conversationDraftKey, readConversationDraft, writeConversationDraft, type ProviderSettingsDraft } from "../../lib/providerSettingsDraft";
import {
  getImageModel,
  getModelProviderProfiles,
  saveImageModel,
  deleteGeneration,
  messageImageToEditSource,
  pickSourceImages,
} from "../../lib/api";
import { consumePendingEditSources } from "../../lib/editSources";
import { consumePendingPrompt } from "../../lib/pendingPrompt";
import {
  completeGenerationMessage,
  failGenerationMessage,
} from "../../lib/generationMessages";
import {
  modelSupportsEdit,
} from "../../lib/generationRequests";
import {
  createUploadedEditSource,
  mergeEditSources,
  normalizePromptFavorite,
} from "../../lib/generatePageHelpers";
import {
  useAcceptPromptAgentDraftMutation,
  useSendPromptAgentMessageMutation,
  useStartPromptAgentSessionMutation,
} from "../promptAgent/queries";
import { capturePreviewSource, useUIStore } from "../../lib/store";
import { useLayoutContext } from "../../components/layout/AppLayout";
import ConfirmDialog from "../../components/common/ConfirmDialog";
import GenerationWorkbench from "../../components/generate/GenerationWorkbench";
import GenerationComposer from "../../components/generate/GenerationComposer";
import GenerationFeed from "../../components/generate/GenerationFeed";
import Lightbox from "../../components/lightbox/Lightbox";
import FolderSelector from "../../components/favorites/FolderSelector";
import { DEFAULT_IMAGE_MODEL, getImageModelCatalogEntry } from "../../lib/modelCatalog";
import type {
  EditSourceImage,
  ComposerMode,
  ImageBackground,
  ImageOutputFormat,
  ImageModel,
  Message,
  MessageImage,
  PromptAgentMessage,
  RetryGenerationRequest,
  ImageAdvancedOptions,
} from "../../types";
import { prepareSubmission, pendingMessages, dispatchSubmission } from "./submission";
import { useTranslation } from "react-i18next";

export default function GenerationSession({ scope, activeProjectId, activeConversationId }: {
  scope: string;
  activeProjectId: string | null;
  activeConversationId: string | null;
}) {
  const { t } = useTranslation();
  const location = useLocation();
  const navigate = useNavigate();
  const draftRevision = useRef(0);
  const submissionPending = useRef(false);
  const unknownOutcomes = useRef(new Set<string>());
  const [pendingUnknownRetry, setPendingUnknownRetry] = useState<RetryGenerationRequest|null>(null);
  const restoringSettingsDraft = useRef(false);
  const redirectedDraft = useRef(false);
  const restoredIntentState = useRef<unknown>(Symbol("no restored intent"));
  const [providerSetup, setProviderSetup] = useState<{model:ImageModel;providerId?:string}|null>(null);
  const {
    setActiveConversationId,
    refreshConversations,
  } = useLayoutContext();
  const { messages, updateMessages: setMessages, isGenerating } = useConversationFeed(activeConversationId, refreshConversations);
  const [prompt, setPrompt] = useState("");
  const [parameters, setParameters] = useState(() => defaultParameters(DEFAULT_IMAGE_MODEL));
  const { size, quality, background, outputFormat, moderation, inputFidelity, imageCount, aspectRatio,
    resolution, outputCompression, advancedOptions } = parameters;
  const patchParameters = useCallback((patch: Partial<ComposerParameters>) => {
    setParameters(current => ({ ...current, ...patch }));
  }, []);
  const setAdvancedOptions = useCallback((change: ImageAdvancedOptions | ((old: ImageAdvancedOptions) => ImageAdvancedOptions)) => {
    setParameters(current => ({
      ...current, advancedOptions: typeof change === "function" ? change(current.advancedOptions) : change,
    }));
  }, []);
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
    if (exact?.api_key_configured) setRecipeWarnings(current => current.includes("provider_missing") ? current.filter(warning => warning !== "provider_missing") : current);
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
  const [agentVisualSelection, setAgentVisualSelection] = useState<string[] | null>(null);
  const [agentTextOnly, setAgentTextOnly] = useState(false);
  const [promptAgentError, setPromptAgentError] = useState<string | null>(null);
  const agentPendingRef = useRef(false);
  const retryAgentAcceptanceRef = useRef<PromptAgentMessage | null>(null);
  const retryAcceptedGenerationRef = useRef(false);
  const agentMountedRef = useRef(true);
  const agentScope = JSON.stringify([location.pathname, activeProjectId, activeConversationId]);
  const agentScopeRef = useRef(agentScope);
  agentScopeRef.current = agentScope;
  useLayoutEffect(() => { agentMountedRef.current = true; return () => { agentMountedRef.current = false; }; }, []);
  const agentVisualSourcePaths = resolveVisualSourcePaths(editSources.map((source) => source.path), agentVisualSelection);
  const [promptAgentSessionId, setPromptAgentSessionId] = useState<
    string | null
  >(null);
  const agentSessionRef = useRef(promptAgentSessionId);
  agentSessionRef.current = promptAgentSessionId;
  const [promptAgentMessages, setPromptAgentMessages] = useState<
    PromptAgentMessage[]
  >([]);
  const previousAgentScopeRef = useRef(agentScope);
  useEffect(() => {
    if (previousAgentScopeRef.current === agentScope) return;
    previousAgentScopeRef.current = agentScope;
    draftRevision.current++;
    if (!restoringSettingsDraft.current) {
      setPromptAgentSessionId(null);
      retryAgentAcceptanceRef.current = null;
      setPromptAgentMessages([]);
      setPromptAgentError(null);
    }
  }, [agentScope]);
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
  const startPromptAgentSessionMutation = useStartPromptAgentSessionMutation();
  const sendPromptAgentMessageMutation = useSendPromptAgentMessageMutation();
  const acceptPromptAgentDraftMutation = useAcceptPromptAgentDraftMutation();

  const { scrollRef, autoScrollRef, chatViewportHeight, handleScroll } = useChatViewport(messages, promptAgentMessages);
  const { promptFavoriteByPrompt, handleTogglePromptFavorite } = usePromptFavorites();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const imageModelRef = useRef(imageModel);
  const didUserSelectModelRef = useRef(false);
  const hasComposerDraftRef = useRef(false);
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
    retryAgentAcceptanceRef.current = null;
    retryAcceptedGenerationRef.current = false;
    setPromptAgentError(null);
  }, []);

  useLayoutEffect(() => {
    const handoff = takeProviderSettingsDraft();
    let saved = readConversationDraft(scope);
    if (handoff) {
      const { projectId, conversationId } = handoff.request;
      // Older settings handoffs did not record either route field.
      const target = projectId === undefined && conversationId === undefined
        ? scope : conversationDraftKey(projectId ?? null, conversationId ?? null);
      writeConversationDraft(target, handoff);
      const canRestoreGlobalConversation = location.pathname === ROUTES.generate
        && (projectId ?? "default") === "default" && !activeConversationId && !!conversationId;
      if (target === scope || canRestoreGlobalConversation) {
        saved = handoff;
        restoredIntentState.current = location.state;
        if (location.state && typeof location.state === "object") {
          consumeNavigationIntent(location.key, location.state, "recipe");
        }
        if (canRestoreGlobalConversation) {
          redirectedDraft.current = true;
          navigate(ROUTES.generate, { replace: true, state: { activateConversationId: conversationId } });
        }
      }
    }
    if (!saved) return;
    // StrictMode can run layout cleanup before the state updates below render.
    latestDraft.current = saved;
    const request = saved.request;
    didUserSelectModelRef.current = true; hasComposerDraftRef.current = true;
    restoringSettingsDraft.current = true; hydratedSourcePath.current = {path: request.editSources[0]?.path};
    setActiveImageModel(request.model);
    setPrompt(request.prompt);
    setParameters(parametersFromRequest(request));
    setEditSources(request.editSources);
    setRecipeIdentity({providerProfileId:request.providerProfileId,providerOverrideConfirmed:request.providerOverrideConfirmed,sourceGenerationId:request.sourceGenerationId});
    setRecipeWarnings(saved.recipeWarnings ?? []); setComposerMode(saved.composerMode ?? "direct");
    setEditingPromptMessageId(saved.editingPromptMessageId);setPromptAgentSessionId(saved.promptAgentSessionId);
    setPromptAgentMessages(saved.promptAgentMessages ?? []);setSelectedPromptAgentConfigId(saved.selectedPromptAgentConfigId ?? "");
    setAgentTextOnly(saved.agentTextOnly ?? false); setAgentVisualSelection(saved.agentVisualSelection ?? null);
  }, [setActiveImageModel]);

  const applyModelDefaults = useCallback((model: ImageModel) => {
    setParameters(defaultParameters(model));
    if (!modelSupportsEdit(model)) setEditSources([]);
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
      if (!consumeNavigationIntent(location.key, location.state, "prompt")) return;
      markComposerDraftStarted();
      setPrompt(navigationPendingPrompt);
      return;
    }

    const pendingPrompt = consumePendingPrompt();
    if (!pendingPrompt) return;

    markComposerDraftStarted();
    setPrompt(pendingPrompt);
  }, [markComposerDraftStarted, navigationPendingPrompt, location.key, location.state]);

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
          setParameters(current => reconcileParameters(current, model));
          if (!modelSupportsEdit(model)) setEditSources([]);
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [applyModelDefaults, setActiveImageModel]);

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
      patchParameters({ background: value });
      if (
        value === "transparent" &&
        !["png", "webp", "svg"].includes(outputFormat)
      ) {
        patchParameters({ outputFormat: "png" });
      }
    },
    [markComposerDraftStarted, outputFormat],
  );

  const handleOutputFormatChange = useCallback(
    (value: ImageOutputFormat) => {
      markComposerDraftStarted();
      patchParameters({ outputFormat: value });
      if (value === "svg") patchParameters({ imageCount: 1 });
      if (value === "gif") {
        patchParameters({ size: "auto" });
        setAdvancedOptions((current) => ({ ...current, maskPath: undefined }));
      }
      if (
        !["png", "webp", "svg"].includes(value) &&
        background === "transparent"
      ) {
        patchParameters({ background: "auto" });
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
      const submittedRevision = draftRevision.current;
      try {
        const saved = await getModelProviderProfiles(request.model);
        if (!agentMountedRef.current) return false;
        const profile = saved.profiles.find(profile => profile.id === (request.providerProfileId ?? saved.active_provider_id));
        if (!profile?.api_key_configured) {
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

      const normalizedRequest = prepareSubmission(request);
      const tempId = normalizedRequest.clientRequestId;
      const pending = pendingMessages(normalizedRequest);
      setMessages(current => [...current, ...pending]);
      autoScrollRef.current = true;

      try {
        const acknowledge = (receipt: { conversation_id: string; status?: string }) => {
          if (!agentMountedRef.current || request.conversationId) return;
          const nextKey = conversationDraftKey(request.projectId ?? null, receipt.conversation_id);
          // Move edits typed while the first task was being accepted to its assigned conversation.
          const changed = draftRevision.current !== submittedRevision;
          const draft = latestDraft.current;
          writeConversationDraft(nextKey, changed || receipt.status === "failed" ? {
            ...draft, request: { ...draft.request, conversationId: receipt.conversation_id },
          } : null);
          promoted.current = true;
          setActiveConversationId(receipt.conversation_id);
          if (request.projectId && request.projectId !== "default" && location.pathname?.startsWith("/projects/")) {
            navigate(ROUTES.projectChat(request.projectId, receipt.conversation_id), { replace: true });
          }
          refreshConversations();
        };
        const result = await dispatchSubmission(normalizedRequest, t, request.conversationId ? undefined : acknowledge);
        if (!agentMountedRef.current) { refreshConversations(); return true; }
        setMessages((prev) => completeGenerationMessage(prev, tempId, result));
        if (!request.conversationId && !promoted.current) acknowledge(result);
        refreshConversations();
        return true;
      } catch (e) {
        if (!agentMountedRef.current) { refreshConversations(); return false; }
        const failureText = e instanceof Error ? e.message : typeof e === "string" ? e : JSON.stringify(e);
        if (["Network", "ProviderOutcomeUnknown"].includes((e as {kind?:string})?.kind ?? "") || /unknown|outcome|timeout|timed out|connection lost|connection reset/i.test(failureText ?? "")) unknownOutcomes.current.add(tempId);
        if (["ApiKeyNotSet", "ProviderProfileNotFound"].includes((e as {kind?:string})?.kind ?? "")) setProviderSetup({model:request.model,providerId:request.providerProfileId});
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
    [refreshConversations, setActiveConversationId, location.pathname, navigate, t],
  );

  const hydrateRecipe = useCallback(
    (recipe: GenerationRecipe) => {
      const request = recipeToRequest(recipe);
      didUserSelectModelRef.current = true;
      markComposerDraftStarted();
      hydratedSourcePath.current = { path: request.editSources[0]?.path };
      setActiveImageModel(request.model);
      setPrompt(request.prompt);
      setParameters(parametersFromRequest(request));
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
      if (!agentMountedRef.current || recipeLoadPending.current) return;
      recipeLoadPending.current = true;
      const token = ++recipeLoadToken.current;
      setRecipeError(null);
      try {
        const loaded = await getGenerationRecipe(generationId);
        const recipe = outcomeUnknown && !loaded.warnings.includes("provider_outcome_unknown")
          ? { ...loaded, warnings: [...loaded.warnings, "provider_outcome_unknown"] }
          : loaded;
        if (!agentMountedRef.current || token !== recipeLoadToken.current) return;
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
  useEffect(() => {
    const state = location.state as {
      recipeGenerationId?: string;
      recipeMode?: "edit" | "generate";
    } | null;
    if (
      state?.recipeGenerationId &&
      consumeNavigationIntent(location.key, state, "recipe")
    ) {
      void handleReuseRecipe(
        state.recipeGenerationId,
        state.recipeMode ?? "edit",
      );
    }
  }, [location.key, location.state, handleReuseRecipe]);

  const currentRequest: RetryGenerationRequest = {
    ...parameters, ...recipeIdentity, prompt, model: imageModel,
    conversationId: activeConversationId, projectId: activeProjectId, editSources,
  };

  async function handleGenerate() {
    const promptText = prompt.trim();
    if (!promptText || recipeWarnings.length > 0) return;

    const revision = draftRevision.current;
    const submitted = await submitGenerationRequest(currentRequest);
    if (submitted && revision === draftRevision.current) {
      setPrompt(""); hasComposerDraftRef.current = false; setEditSources([]); setEditingPromptMessageId(null);
    }
  }

  const handleDeepThinkingSend = useCallback(async () => {
    const trimmed = prompt.trim();
    if (!trimmed || isGenerating || agentPendingRef.current) return;
    if (!selectedPromptAgentConfigId) {
      setPromptAgentError(t("generate.agent.selectLlm"));
      return;
    }
    const sourceImagePaths = [...new Set(editSources.map((source) => source.path))];
    const visualSourceImagePaths = agentTextOnly ? [] : resolveVisualSourcePaths(sourceImagePaths, agentVisualSelection);
    if (sourceImagePaths.length && !agentTextOnly && (visualSourceImagePaths.length === 0 || visualSourceImagePaths.length > 3)) {
      setPromptAgentError(t("promptVision.selectUpToThree"));
      return;
    }
    const revision = draftRevision.current;
    const scope = agentScopeRef.current;
    const isCurrent = () => agentMountedRef.current && agentScopeRef.current === scope && draftRevision.current === revision;
    agentPendingRef.current = true;
    setPromptAgentError(null);
    try {
      const common = { configId: selectedPromptAgentConfigId, sourceImagePaths, visualSourceImagePaths, textOnly: agentTextOnly };
      const result = promptAgentSessionId
        ? await sendPromptAgentMessageMutation.mutateAsync({ ...common, sessionId: promptAgentSessionId, message: trimmed })
        : await startPromptAgentSessionMutation.mutateAsync({ ...common, prompt: trimmed, conversationId: activeConversationId, projectId: activeProjectId });
      if (!isCurrent()) return;
      setPromptAgentSessionId(result.session.id);
      setPromptAgentMessages(result.messages);
      setPrompt("");
    } catch (error) {
      if (isCurrent()) setPromptAgentError(t(promptVisionErrorKey(error)));
    } finally {
      agentPendingRef.current = false;
    }
  }, [activeConversationId, activeProjectId, editSources, isGenerating, prompt, promptAgentSessionId,
    selectedPromptAgentConfigId, sendPromptAgentMessageMutation, startPromptAgentSessionMutation,
    agentTextOnly, agentVisualSelection, t]);

  const handleAcceptAgentDraft = useCallback(
    async (message: PromptAgentMessage) => {
      const draft = message.draft_prompt?.trim();
      if (!draft || !promptAgentSessionId || recipeWarnings.length > 0 || isGenerating || agentPendingRef.current) return;
      const revision = draftRevision.current;
      const scope = agentScopeRef.current;
      const isCurrent = () => agentMountedRef.current && agentScopeRef.current === scope && draftRevision.current === revision;
      agentPendingRef.current = true;
      setPromptAgentError(null);
      let acceptanceSucceeded = false;
      try {

      const accepted = await acceptPromptAgentDraftMutation.mutateAsync({
        sessionId: promptAgentSessionId,
        acceptedPrompt: draft,
      });
      acceptanceSucceeded = true;
      if (!isCurrent()) {
        // Acceptance is durable even when its response is stale. Do not reuse
        // that inactive session for the user's newer draft.
        if (agentMountedRef.current && agentScopeRef.current === scope && agentSessionRef.current === promptAgentSessionId) {
          agentSessionRef.current = null;
          setPromptAgentSessionId(null);
          setPromptAgentMessages([]);
        }
        return;
      }
      retryAgentAcceptanceRef.current = null;
      retryAcceptedGenerationRef.current = false;
      const acceptedPrompt = accepted.accepted_prompt ?? draft;
      const nextRequest = applyAgentSuggestions(currentRequest, acceptedPrompt, message.suggested_params);
      setPrompt(nextRequest.prompt);
      setActiveImageModel(nextRequest.model);
      setParameters(parametersFromRequest(nextRequest));
      setComposerMode("direct");
      setPromptAgentMessages([]);
      setPromptAgentSessionId(null);

      await submitGenerationRequest(nextRequest);
      } catch (error) {
        if (isCurrent()) {
          retryAgentAcceptanceRef.current = acceptanceSucceeded ? null : message;
          retryAcceptedGenerationRef.current = acceptanceSucceeded;
          setPromptAgentError(t(promptVisionErrorKey(error)));
        }
      } finally {
        agentPendingRef.current = false;
      }
    },
    [
      isGenerating, t, acceptPromptAgentDraftMutation, recipeWarnings, promptAgentSessionId,
      currentRequest, setActiveImageModel, submitGenerationRequest,
    ],
  );

  const handleContinueAgentDraft = useCallback(
    (message: PromptAgentMessage) => {
      markComposerDraftStarted();
      setPrompt(message.draft_prompt ?? "");
      textareaRef.current?.focus();
    },
    [markComposerDraftStarted],
  );

  const handleEditAgentDraft = useCallback((message: PromptAgentMessage) => {
    markComposerDraftStarted();
    setPrompt(message.draft_prompt ?? "");
    setComposerMode("direct");
    textareaRef.current?.focus();
  }, [markComposerDraftStarted]);

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

  const configuredProfile = providerQuery.data?.profiles.find(profile => profile.id === (recipeIdentity.providerProfileId ?? providerQuery.data?.active_provider_id));
  const needsProviderSetup = !!providerSetup || (!!providerQuery.data && !configuredProfile?.api_key_configured) || providerQuery.isError;
  const draft: ProviderSettingsDraft = {
    request: currentRequest,
    recipeWarnings, composerMode, editingPromptMessageId, promptAgentSessionId, promptAgentMessages,
    selectedPromptAgentConfigId, agentTextOnly, agentVisualSelection,
  };
  const latestDraft = useRef(draft);
  const promoted = useRef(false);
  latestDraft.current = draft;
  useLayoutEffect(() => () => {
    if (redirectedDraft.current) return;
    const saved = latestDraft.current;
    const hasDraft = hasComposerDraftRef.current || saved.request.prompt || saved.request.editSources.length || saved.promptAgentSessionId;
    writeConversationDraft(scope, hasDraft && !promoted.current ? saved : null);
  }, [scope]);

  function openProviderSettings() {
    if (agentPendingRef.current) return;
    try {
      saveProviderSettingsDraft(latestDraft.current);
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
                sendPromptAgentMessageMutation.isPending || acceptPromptAgentDraftMutation.isPending
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
          {needsProviderSetup && <div role="status" className="flex flex-wrap items-center justify-between gap-2 border-t border-border-subtle px-5 py-3 text-sm"><span className="text-muted">{t("generate.providerSetupRequired")}</span><button type="button" disabled={startPromptAgentSessionMutation.isPending || sendPromptAgentMessageMutation.isPending || acceptPromptAgentDraftMutation.isPending} onClick={openProviderSettings} className="studio-control focus-ring min-h-9 rounded-lg px-3 py-2 text-xs font-medium">{t("generate.configureProvider")}</button></div>}
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
              patchParameters({ outputCompression: value });
            }}
            textareaRef={textareaRef}
            prompt={prompt}
            imageModel={imageModel}
            size={size}
            aspectRatio={aspectRatio}
            resolution={resolution}
            onSizeChange={(value) => {
              markComposerDraftStarted();
              patchParameters({ size: value });
            }}
            onAspectRatioChange={(value) => {
              markComposerDraftStarted();
              patchParameters({ aspectRatio: value });
            }}
            onResolutionChange={(value) => {
              markComposerDraftStarted();
              patchParameters({ resolution: value });
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
              patchParameters({ quality: value });
            }}
            onBackgroundChange={handleBackgroundChange}
            onOutputFormatChange={handleOutputFormatChange}
            onModerationChange={(value) => {
              markComposerDraftStarted();
              patchParameters({ moderation: value });
            }}
            onInputFidelityChange={(value) => {
              markComposerDraftStarted();
              patchParameters({ inputFidelity: value });
            }}
            onImageCountChange={(value) => {
              markComposerDraftStarted();
              patchParameters({ imageCount: value });
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
            onComposerModeChange={(value) => { markComposerDraftStarted(); setComposerMode(value); }}
            onDeepThinkingSend={() => void handleDeepThinkingSend()}
            promptAgentConfigId={selectedPromptAgentConfigId}
            onPromptAgentConfigChange={(configId, userInitiated) => {
              if (userInitiated && configId !== selectedPromptAgentConfigId) markComposerDraftStarted();
              setSelectedPromptAgentConfigId(configId);
            }}
            agentVisualSourcePaths={agentVisualSourcePaths}
            onAgentVisualSourcePathsChange={(paths) => { markComposerDraftStarted(); setAgentVisualSelection(paths); }}
            agentTextOnly={agentTextOnly}
            onAgentTextOnlyChange={(value) => { markComposerDraftStarted(); setAgentTextOnly(value); }}
            promptAgentError={promptAgentError}
            onRetryPromptAgent={() => { const message = retryAgentAcceptanceRef.current; void (message ? handleAcceptAgentDraft(message) : retryAcceptedGenerationRef.current ? handleGenerate() : handleDeepThinkingSend()); }}
            isPromptAgentRunning={
              startPromptAgentSessionMutation.isPending ||
              sendPromptAgentMessageMutation.isPending || acceptPromptAgentDraftMutation.isPending
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
