import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import GeneratePage from "./GeneratePage";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});

function TestWrapper({ children }: { children: React.ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

const getConversationGenerations = vi.fn();
const getImageModel = vi.fn();
const getModelProviderProfiles = vi.fn();
const navigate = vi.fn();
const getGenerationRecipe = vi.fn();
const listRecipeProviders = vi.fn();
vi.mock("../lib/recipeApi", async () => ({
  ...(await vi.importActual<typeof import("../lib/recipeApi")>(
    "../lib/recipeApi",
  )),
  getGenerationRecipe: (...args: unknown[]) => getGenerationRecipe(...args),
  listRecipeProviders: (...args: unknown[]) => listRecipeProviders(...args),
}));
const saveImageModel = vi.fn();
const generateImage = vi.fn();
const editImage = vi.fn();
const pickSourceImages = vi.fn();
const onGenerationComplete = vi.fn();
const onGenerationFailed = vi.fn();
const onGenerationPartial = vi.fn().mockResolvedValue(() => {});
const createPromptFavorite = vi.fn();
const getPromptFavorites = vi.fn();
const deletePromptFavorite = vi.fn();
const getLlmConfigs = vi.fn().mockResolvedValue([]);
const optimizePrompt = vi.fn();
const startPromptAgentSession = vi.fn();
const sendPromptAgentMessage = vi.fn();
const acceptPromptAgentDraft = vi.fn();
const cancelPromptAgentSession = vi.fn();
const getPromptAgentSession = vi.fn();
const consumePendingPrompt = vi.fn();
const useLocation = vi.fn<() => { state: unknown }>(() => ({ state: null }));
let activeConversationId = "conversation-1";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) =>
      ({
        "generate.placeholder": "Describe the image you want to generate...",
        "generate.editPlaceholder":
          "Describe how you want to edit these source images...",
        "generate.modelLabel": "Model",
        "generate.sizeLabel": "Size",
        "generate.aspectRatioLabel": "Aspect ratio",
        "generate.resolutionLabel": "Resolution",
        "generate.qualityLabel": "Quality",
        "generate.countLabel": "Count",
        "generate.formatLabel": "Format",
        "generate.backgroundLabel": "Background",
        "generate.moderationLabel": "Moderation",
        "generate.compressionLabel": "Compression",
        "generate.inputFidelityLabel": "Input fidelity",
        "generate.llm.optimize": "Optimize",
        "generate.llm.optimizeTitle": "Optimize prompt with AI",
        "generate.llm.optimizeWithImages": "Optimize with image context",
        "generate.llm.emptyPrompt": "Enter a prompt before optimizing.",
        "generate.llm.noPromptConfig":
          "Enable a text or multimodal LLM in Settings before optimizing prompts.",
        "generate.llm.noMultimodalConfig":
          "Enable a multimodal LLM in Settings before optimizing with image context.",
        "generate.llm.selectConfigFirst": "Select an LLM before optimizing.",
        "generate.llm.generating":
          "Wait for the current generation to finish before optimizing.",
        "generate.parametersLabel": "Generation parameters",
        "generate.submit": "Generate",
        "generate.mode.direct": "Direct",
        "generate.mode.deepThinking": "Deep thinking",
        "generate.agent.selectLlm":
          "Select an enabled LLM before using deep thinking mode.",
        "generate.agent.thinking": "Thinking through the prompt...",
        "generate.agent.finalPrompt": "Final prompt",
        "generate.agent.acceptAndGenerate": "Accept and generate",
        "generate.agent.continueRefining": "Continue refining",
        "generate.agent.editManually": "Edit manually",
        "generate.uploadSource": "Upload Source",
        "generate.clearSources": "Clear sources",
        "generate.editingPrompt": "Editing previous prompt",
        "generate.cancelEditPrompt": "Cancel edit",
        "generate.favoritePrompt": "Favorite prompt",
        "generate.removePromptFavorite": "Remove prompt favorite",
        "generate.promptFavorites": "Prompt favorites",
        "generate.promptFavoritesCount": `${options?.count} saved`,
        "generate.noPromptFavorites": "No prompt favorites yet",
        "generate.deletePromptFavorite": "Delete prompt favorite",
        "generate.closePromptFavorites": "Close prompt favorites",
        "generate.editingSources":
          options?.count === 1
            ? "Editing 1 source image"
            : `Editing ${options?.count} source images`,
        "generate.editPrompt": "Edit prompt",
        "generate.auto": "Auto",
        "generate.square": "Square",
        "generate.landscape": "Landscape",
        "generate.portrait": "Portrait",
        "generate.size.square1k": "Square 1K",
        "generate.size.landscape1k": "Landscape 1K",
        "generate.size.portrait1k": "Portrait 1K",
        "generate.size.square2k": "Square 2K",
        "generate.size.landscape2k": "Landscape 2K",
        "generate.size.landscape4k": "Landscape 4K",
        "generate.size.portrait4k": "Portrait 4K",
        "generate.quality.auto": "Auto",
        "generate.quality.high": "High",
        "generate.quality.medium": "Medium",
        "generate.quality.low": "Low",
        "generate.countValue": `${options?.count} images`,
        "generate.format.png": "PNG",
        "generate.format.jpeg": "JPEG",
        "generate.format.webp": "WEBP",
        "generate.background.auto": "Auto",
        "generate.background.opaque": "Opaque",
        "generate.background.transparent": "Transparent",
        "generate.moderation.auto": "Auto",
        "generate.moderation.low": "Low",
        "generate.inputFidelity.high": "High",
        "generate.inputFidelity.low": "Low",
      })[key] ?? key,
  }),
}));

vi.mock("../lib/api", () => ({
  getModelProviderProfiles: (...args: unknown[]) => getModelProviderProfiles(...args),
  createPromptFavorite: (...args: unknown[]) => createPromptFavorite(...args),
  deleteGeneration: vi.fn(),
  deletePromptFavorite: (...args: unknown[]) => deletePromptFavorite(...args),
  editImage: (...args: unknown[]) => editImage(...args),
  generateImage: (...args: unknown[]) => generateImage(...args),
  getConversationGenerations: (...args: unknown[]) =>
    getConversationGenerations(...args),
  getImageModel: (...args: unknown[]) => getImageModel(...args),
  getPromptFavorites: (...args: unknown[]) => getPromptFavorites(...args),
  getLlmConfigs: (...args: unknown[]) => getLlmConfigs(...args),
  optimizePrompt: (...args: unknown[]) => optimizePrompt(...args),
  startPromptAgentSession: (...args: unknown[]) =>
    startPromptAgentSession(...args),
  sendPromptAgentMessage: (...args: unknown[]) =>
    sendPromptAgentMessage(...args),
  acceptPromptAgentDraft: (...args: unknown[]) =>
    acceptPromptAgentDraft(...args),
  cancelPromptAgentSession: (...args: unknown[]) =>
    cancelPromptAgentSession(...args),
  getPromptAgentSession: (...args: unknown[]) => getPromptAgentSession(...args),
  onGenerationComplete: (...args: unknown[]) => onGenerationComplete(...args),
  onGenerationPartial: (...args: unknown[]) => onGenerationPartial(...args),
  onGenerationFailed: (...args: unknown[]) => onGenerationFailed(...args),
  saveImageModel: (...args: unknown[]) => saveImageModel(...args),
  messageImageToEditSource: (image: {
    path: string;
    imageId?: string;
    generationId?: string;
  }) => ({
    id: `${image.imageId ?? image.generationId ?? "source"}:${image.path}`,
    path: image.path,
    label: image.path.split("/").pop() ?? "source-image",
    imageId: image.imageId,
    generationId: image.generationId,
  }),
  pickSourceImages: (...args: unknown[]) => pickSourceImages(...args),
  toAssetUrl: (path: string) => path,
}));

vi.mock("../lib/editSources", () => ({
  consumePendingEditSources: () => [],
}));

vi.mock("../lib/pendingPrompt", () => ({
  consumePendingPrompt: () => consumePendingPrompt(),
}));

vi.mock("react-router-dom", async () => {
  const actual =
    await vi.importActual<typeof import("react-router-dom")>(
      "react-router-dom",
    );
  return {
    ...actual,
    useLocation: () => useLocation(),
    useNavigate: () => navigate,
  };
});

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    activeProjectId: "project-1",
    activeConversationId,
    setActiveConversationId: vi.fn(),
    refreshConversations: vi.fn(),
  }),
}));

vi.mock("../components/common/ConfirmDialog", () => ({
  default: ({
    open,
    confirmLabel,
    onConfirm,
    onCancel,
  }: {
    open: boolean;
    confirmLabel: string;
    onConfirm: () => void;
    onCancel: () => void;
  }) =>
    open ? (
      <div role="dialog">
        <button onClick={onConfirm}>{confirmLabel}</button>
        <button onClick={onCancel}>Cancel dialog</button>
      </div>
    ) : null,
}));

vi.mock("../components/generate/MessageBubble", () => ({
  default: ({
    message,
    onEditImage,
    onEditPrompt,
    onFavoritePrompt,
    isPromptFavorited,
    onRetry,
  }: {
    message: {
      id: string;
      role: string;
      content: string;
      status?: string;
      images?: Array<{
        path: string;
        imageId: string;
        generationId: string;
      }>;
    };
    onEditImage?: (image: {
      path: string;
      imageId: string;
      generationId: string;
    }) => void;
    onEditPrompt?: (message: {
      id: string;
      role: string;
      content: string;
    }) => void;
    onFavoritePrompt?: (message: {
      id: string;
      role: string;
      content: string;
    }) => void;
    isPromptFavorited?: boolean;
    onRetry?: (message: unknown) => void;
  }) =>
    message.role === "user" ? (
      <div>
        <button onClick={() => onEditPrompt?.(message)}>Edit prompt</button>
        <button onClick={() => onFavoritePrompt?.(message)}>
          {isPromptFavorited ? "Remove prompt favorite" : "Favorite prompt"}
        </button>
      </div>
    ) : message.images?.[0] ? (
      <div>
        <span>{message.images[0].path}</span>
        <button onClick={() => onEditImage?.(message.images![0])}>
          Edit image
        </button>
      </div>
    ) : message.status === "failed" ? <button onClick={() => onRetry?.(message)}>Retry failed generation</button> : null,
}));

vi.mock("../components/lightbox/Lightbox", () => ({
  default: () => null,
}));

vi.mock("../components/favorites/FolderSelector", () => ({
  default: () => null,
}));

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

describe("GeneratePage", () => {
  it("reviews persisted failed recipes before retrying an unknown provider outcome", async () => {
    getConversationGenerations.mockResolvedValue([{generation:{id:"failed-saved",prompt:"Saved failed prompt",engine:"gpt-image-2",status:"failed",size:"1024x1024",quality:"high",background:"auto",output_format:"png",moderation:"auto",input_fidelity:"high",image_count:1,source_image_paths:[],created_at:"2026-11-13",request_metadata:JSON.stringify({model:"gpt-image-2",project_id:"project-1",conversation_id:"conversation-1"})},images:[]}]);
    getGenerationRecipe.mockResolvedValue({version:1,generation_id:"failed-saved",provenance:"job",prompt:"Saved failed prompt",model:"gpt-image-2",request_kind:"generate",provider_profile_id:"saved-profile",source_image_paths:[],warnings:["provider_outcome_unknown"],options:{size:"1024x1024",quality:"high",image_count:1}});
    render(<GeneratePage/>,{wrapper:TestWrapper});
    fireEvent.click(await screen.findByRole("button",{name:"Retry failed generation"}));
    await waitFor(()=>expect(getGenerationRecipe).toHaveBeenCalledWith("failed-saved"));
    expect(generateImage).not.toHaveBeenCalled();
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
  });

  it("requires explicit confirmation before retrying an in-memory unknown network outcome", async () => {
    generateImage.mockRejectedValue({kind:"Network",detail:{message:"connection lost"}});
    render(<GeneratePage/>,{wrapper:TestWrapper});
    fireEvent.change(screen.getByPlaceholderText("Describe the image you want to generate..."),{target:{value:"Unknown result prompt"}});
    fireEvent.click(screen.getByRole("button",{name:"Generate"}));
    fireEvent.click(await screen.findByRole("button",{name:"Retry failed generation"}));
    expect(generateImage).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(screen.getByRole("button",{name:"generate.confirmUnknownRetry"})).toBeInTheDocument();
  });

  it("keeps a full recipe draft through provider settings without generating automatically", async () => {
    getModelProviderProfiles.mockResolvedValue({active_provider_id:"saved-profile",profiles:[]});
    getGenerationRecipe.mockResolvedValue({version:1,generation_id:"saved",provenance:"job",prompt:"Keep this draft",model:"gpt-image-2.6-flare",request_kind:"edit",provider_profile_id:"saved-profile",source_image_paths:["/tmp/first.png","/tmp/second.png"],warnings:[],options:{size:"1024x1024",quality:"high",background:"opaque",output_format:"webp",output_compression:73,moderation:"auto",input_fidelity:"high",image_count:2,advanced_options:{markupPath:"/tmp/overlay.png",complexity:"high"}}});
    useLocation.mockReturnValue({state:{recipeGenerationId:"saved",recipeMode:"edit"}});
    const view=render(<GeneratePage/>,{wrapper:TestWrapper});
    await screen.findByDisplayValue("Keep this draft");
    fireEvent.click(screen.getByRole("button",{name:"Generate"}));
    await waitFor(()=>expect(getModelProviderProfiles).toHaveBeenCalled());
    expect(screen.getByDisplayValue("Keep this draft")).toBeInTheDocument();
    expect(editImage).not.toHaveBeenCalled();expect(generateImage).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button",{name:"generate.configureProvider"}));
    expect(navigate).toHaveBeenCalledWith(expect.stringContaining("model=gpt-image-2.6-flare"));
    view.unmount();useLocation.mockReturnValue({state:{recipeGenerationId:"saved",recipeMode:"generate"}});queryClient.clear();
    getModelProviderProfiles.mockResolvedValue({active_provider_id:"saved-profile",profiles:[{id:"saved-profile",name:"Configured",api_key:"fixture",endpoint_settings:{mode:"base_url",base_url:"https://example.test/v1",generation_url:"",edit_url:""}}]});
    render(<GeneratePage/>,{wrapper:TestWrapper});
    await screen.findByDisplayValue("Keep this draft");expect(editImage).not.toHaveBeenCalled();
    await waitFor(()=>expect(screen.getByRole("button",{name:"Generate"})).not.toBeDisabled());
    fireEvent.click(screen.getByRole("button",{name:"Generate"}));
    await waitFor(()=>expect(editImage).toHaveBeenCalledWith(expect.objectContaining({prompt:"Keep this draft",model:"gpt-image-2.6-flare",providerProfileId:"saved-profile",sourceGenerationId:"saved",sourceImagePaths:["/tmp/first.png","/tmp/second.png"],quality:"high",imageCount:2,outputCompression:73,advancedOptions:{markupPath:"/tmp/overlay.png",complexity:"high"}})));
  });

  it("requires an explicit replacement when the recipe provider was deleted", async () => {
    getGenerationRecipe.mockResolvedValue({version:1,generation_id:"saved",provenance:"job",prompt:"Keep deleted provider draft",model:"gpt-image-2",request_kind:"generate",provider_profile_id:"deleted-profile",source_image_paths:[],warnings:[],options:{size:"1024x1024",quality:"high",image_count:1}});
    useLocation.mockReturnValue({state:{recipeGenerationId:"saved",recipeMode:"edit"}});
    render(<GeneratePage/>,{wrapper:TestWrapper});
    await screen.findByDisplayValue("Keep deleted provider draft");
    fireEvent.click(screen.getByRole("button",{name:"Generate"}));
    const picker=await screen.findByLabelText("recipe.providerLabel");
    expect(generateImage).not.toHaveBeenCalled();
    expect(screen.getByDisplayValue("Keep deleted provider draft")).toBeInTheDocument();
    await waitFor(()=>expect(picker).toHaveValue("saved-profile"));
    fireEvent.change(picker,{target:{value:"another"}});
    fireEvent.click(screen.getByRole("button",{name:"recipe.confirmProvider"}));
    expect(screen.queryByRole("button",{name:"generate.configureProvider"})).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Generate"}));
    await waitFor(()=>expect(generateImage).toHaveBeenCalledWith(expect.objectContaining({providerProfileId:"another",providerOverrideConfirmed:true,sourceGenerationId:"saved",prompt:"Keep deleted provider draft"})));
  });

  beforeEach(() => {
    queryClient.clear();
    sessionStorage.clear();
    navigate.mockReset();
    getModelProviderProfiles.mockReset();
    getModelProviderProfiles.mockResolvedValue({active_provider_id:"saved-profile",profiles:[{id:"saved-profile",name:"Saved",api_key:"fixture",endpoint_settings:{mode:"base_url",base_url:"https://example.test/v1",generation_url:"",edit_url:""}},{id:"another",name:"Another",api_key:"fixture",endpoint_settings:{mode:"base_url",base_url:"https://example.test/v1",generation_url:"",edit_url:""}}]});
    getConversationGenerations.mockReset();
    getImageModel.mockReset();
    getGenerationRecipe.mockReset();
    listRecipeProviders.mockResolvedValue([
      { id: "saved-profile", name: "Saved Provider" },
      { id: "another", name: "Another Provider" },
    ]);
    saveImageModel.mockReset();
    generateImage.mockReset();
    editImage.mockReset();
    pickSourceImages.mockReset();
    onGenerationComplete.mockReset();
    onGenerationFailed.mockReset();
    createPromptFavorite.mockReset();
    createPromptFavorite.mockImplementation(async (prompt: string) => ({
      id: `fav-${crypto.randomUUID()}`,
      prompt,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }));
    getPromptFavorites.mockReset();
    getLlmConfigs.mockReset();
    optimizePrompt.mockReset();
    startPromptAgentSession.mockReset();
    sendPromptAgentMessage.mockReset();
    acceptPromptAgentDraft.mockReset();
    cancelPromptAgentSession.mockReset();
    getPromptAgentSession.mockReset();
    deletePromptFavorite.mockReset();
    consumePendingPrompt.mockReset();
    useLocation.mockReset();
    useLocation.mockReturnValue({ state: null });
    activeConversationId = "conversation-1";

    getConversationGenerations.mockResolvedValue([
      {
        generation: {
          id: "generation-1",
          prompt: "A dramatic mountain temple at sunrise",
          created_at: "2026-04-26T00:00:00Z",
          status: "completed",
          source_image_paths: [],
        },
        images: [
          {
            id: "image-1",
            generation_id: "generation-1",
            file_path: "/tmp/source.png",
            thumbnail_path: "/tmp/source-thumb.png",
          },
        ],
      },
    ]);
    getImageModel.mockResolvedValue("gpt-image-2");
    getPromptFavorites.mockResolvedValue([]);
    getLlmConfigs.mockResolvedValue([]);
    optimizePrompt.mockResolvedValue(
      "A cinematic moonlit observatory with refined lighting.",
    );
    startPromptAgentSession.mockResolvedValue({
      session: {
        id: "agent-session-1",
        status: "active",
        original_prompt: "A prompt",
        draft_prompt: null,
        accepted_prompt: null,
        selected_skill_ids: [],
        suggested_params: {},
        created_at: "2026-05-16T00:00:00Z",
        updated_at: "2026-05-16T00:00:00Z",
      },
      messages: [],
    });
    sendPromptAgentMessage.mockResolvedValue({
      session: {
        id: "agent-session-1",
        status: "active",
        original_prompt: "A prompt",
        draft_prompt: null,
        accepted_prompt: null,
        selected_skill_ids: [],
        suggested_params: {},
        created_at: "2026-05-16T00:00:00Z",
        updated_at: "2026-05-16T00:00:00Z",
      },
      messages: [],
    });
    acceptPromptAgentDraft.mockResolvedValue({
      id: "agent-session-1",
      status: "accepted",
      original_prompt: "A prompt",
      draft_prompt: "Final prompt",
      accepted_prompt: "Final prompt",
      selected_skill_ids: [],
      suggested_params: {},
      created_at: "2026-05-16T00:00:00Z",
      updated_at: "2026-05-16T00:00:00Z",
    });
    cancelPromptAgentSession.mockResolvedValue({});
    getPromptAgentSession.mockResolvedValue({ session: {}, messages: [] });
    createPromptFavorite.mockImplementation(async (prompt: string) => ({
      id: "favorite-1",
      prompt,
      created_at: "2026-04-28T00:00:00Z",
      updated_at: "2026-04-28T00:00:00Z",
    }));
    deletePromptFavorite.mockResolvedValue(undefined);
    saveImageModel.mockResolvedValue(undefined);
    generateImage.mockResolvedValue({
      generation_id: "generation-new",
      conversation_id: "conversation-1",
      images: [],
    });
    editImage.mockResolvedValue({
      generation_id: "generation-edit",
      conversation_id: "conversation-1",
      images: [],
    });
    pickSourceImages.mockResolvedValue([]);
    onGenerationComplete.mockResolvedValue(vi.fn());
    onGenerationFailed.mockResolvedValue(vi.fn());
    consumePendingPrompt.mockReturnValue(null);
  });

  it("hydrates a saved edit atomically and preserves overlays despite a late preferred-model response", async () => {
    let resolveModel!: (model: string) => void;
    getImageModel.mockReturnValue(
      new Promise<string>((resolve) => {
        resolveModel = resolve;
      }),
    );
    getGenerationRecipe.mockResolvedValue({
      version: 1,
      generation_id: "saved",
      provenance: "job",
      prompt: "Keep recipe settings",
      model: "gpt-image-2.6-flare",
      request_kind: "edit",
      provider_profile_id: "saved-profile",
      source_image_paths: ["/tmp/ref.png"],
      warnings: [],
      options: {
        size: "auto",
        quality: "high",
        background: "opaque",
        output_format: "webp",
        output_compression: 73,
        moderation: "auto",
        input_fidelity: "high",
        image_count: 1,
        advanced_options: {
          markupPath: "/tmp/overlay.png",
          complexity: "high",
        },
      },
    });
    useLocation.mockReturnValue({
      state: { recipeGenerationId: "saved", recipeMode: "edit" },
    });
    render(<GeneratePage />, { wrapper: TestWrapper });
    await waitFor(() =>
      expect(
        screen.getByDisplayValue("Keep recipe settings"),
      ).toBeInTheDocument(),
    );
    await act(async () => resolveModel("gpt-image-2"));
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "gpt-image-2.6-flare",
          providerProfileId: "saved-profile",
          sourceGenerationId: "saved",
          outputCompression: 73,
          sourceImagePaths: ["/tmp/ref.png"],
          advancedOptions: {
            markupPath: "/tmp/overlay.png",
            complexity: "high",
          },
        }),
      ),
    );
    expect(saveImageModel).not.toHaveBeenCalled();
  });

  it("does not directly generate a recipe with a changed provider endpoint", async () => {
    getGenerationRecipe.mockResolvedValue({
      version: 1,
      generation_id: "saved",
      provenance: "job",
      prompt: "Recipe",
      model: "gpt-image-2",
      request_kind: "generate",
      source_image_paths: [],
      warnings: ["endpoint_changed"],
      options: {},
    });
    useLocation.mockReturnValue({
      state: { recipeGenerationId: "saved", recipeMode: "generate" },
    });
    render(<GeneratePage />, { wrapper: TestWrapper });
    await waitFor(() =>
      expect(getGenerationRecipe).toHaveBeenCalledWith("saved"),
    );
    expect(generateImage).not.toHaveBeenCalled();
    expect(editImage).not.toHaveBeenCalled();
    fireEvent.click(
      await screen.findByRole("button", { name: "recipe.reviewParameters" }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("recipe.providerLabel")).toHaveValue(
        "saved-profile",
      ),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "recipe.confirmProvider" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          providerProfileId: "saved-profile",
          providerOverrideConfirmed: true,
          sourceGenerationId: "saved",
        }),
      ),
    );
  });

  it("keeps the saved provider when resetting unsupported recipe parameters", async () => {
    getGenerationRecipe.mockResolvedValue({
      version: 1,
      generation_id: "saved",
      provenance: "job",
      prompt: "Saved prompt",
      model: "gpt-image-2",
      provider_profile_id: "saved-profile",
      request_kind: "generate",
      source_image_paths: [],
      warnings: ["invalid_parameters"],
      options: {},
    });
    useLocation.mockReturnValue({
      state: { recipeGenerationId: "saved", recipeMode: "edit" },
    });
    render(<GeneratePage />, { wrapper: TestWrapper });
    fireEvent.click(
      await screen.findByRole("button", { name: "recipe.reviewParameters" }),
    );
    fireEvent.click(
      await screen.findByRole("button", { name: "recipe.resetParameters" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          providerProfileId: "saved-profile",
          sourceGenerationId: "saved",
        }),
      ),
    );
  });

  it("requires acknowledgement of possible duplicate charges before regenerating an unknown outcome", async () => {
    getGenerationRecipe.mockResolvedValue({
      version: 1,
      generation_id: "unknown",
      provenance: "job",
      prompt: "Saved prompt",
      model: "gpt-image-2",
      provider_profile_id: "saved-profile",
      request_kind: "generate",
      source_image_paths: [],
      warnings: ["provider_outcome_unknown"],
      options: {},
    });
    useLocation.mockReturnValue({
      state: { recipeGenerationId: "unknown", recipeMode: "generate" },
    });
    render(<GeneratePage />, { wrapper: TestWrapper });
    fireEvent.click(
      await screen.findByRole("button", { name: "recipe.reviewParameters" }),
    );
    expect(generateImage).not.toHaveBeenCalled();
    fireEvent.click(
      await screen.findByRole("button", {
        name: "recipe.acknowledgeNewCharge",
      }),
    );
    expect(generateImage).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() => expect(generateImage).toHaveBeenCalledTimes(1));
  });

  it("requires confirmation before replacing an existing composer draft", async () => {
    const { rerender } = render(<GeneratePage />, { wrapper: TestWrapper });
    await screen.findByLabelText("Model");
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "Unsaved prompt" } },
    );
    getGenerationRecipe.mockResolvedValue({
      version: 1,
      generation_id: "saved",
      provenance: "job",
      prompt: "Saved prompt",
      model: "gpt-image-2",
      request_kind: "generate",
      source_image_paths: [],
      warnings: [],
      options: {},
    });
    useLocation.mockReturnValue({
      state: { recipeGenerationId: "saved", recipeMode: "edit" },
    });
    rerender(<GeneratePage />);
    await screen.findByRole("dialog");
    expect(screen.getByDisplayValue("Unsaved prompt")).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "recipe.reviewParameters" }),
    );
    expect(screen.getByDisplayValue("Saved prompt")).toBeInTheDocument();
  });

  it("renders every registered model from the shared catalog in the model selector", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    const { IMAGE_MODEL_CATALOG } = await import("../lib/modelCatalog");
    const modelSelect = await screen.findByLabelText("Model");
    const optionNames = within(modelSelect)
      .getAllByRole("option")
      .map((option) => option.textContent);

    expect(optionNames).toEqual(
      IMAGE_MODEL_CATALOG.map((entry) => entry.i18nKey),
    );
  });

  it("submits the pixel size selected in the composer", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Size"), {
      target: { value: "1536x1024" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A quiet moonlit observatory" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "gpt-image-2",
          prompt: "A quiet moonlit observatory",
        }),
      );
    });
    expect(generateImage.mock.calls[0][0]).toHaveProperty("size", "1536x1024");
  });

  it("loads a sent prompt back into the composer when editing", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Edit prompt" }));

    expect(
      screen.getByDisplayValue("A dramatic mountain temple at sunrise"),
    ).toBeInTheDocument();
    expect(screen.getByText("Editing previous prompt")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Cancel edit" }),
    ).toBeInTheDocument();
  });

  it("keeps the prompt optimize button visible when no LLM config is enabled", async () => {
    getLlmConfigs.mockResolvedValue([]);

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A quiet moonlit observatory" } },
    );

    const optimizeButton = screen.getByRole("button", { name: "Optimize" });
    expect(optimizeButton).toBeInTheDocument();
    expect(optimizeButton).not.toBeDisabled();
    fireEvent.click(optimizeButton);

    expect(
      screen.getByText(
        "Enable a text or multimodal LLM in Settings before optimizing prompts.",
      ),
    ).toBeInTheDocument();
  });

  it("optimizes text prompts with an enabled multimodal LLM config", async () => {
    getLlmConfigs.mockResolvedValue([
      {
        id: "vision-helper",
        name: "Vision Helper",
        protocol: "openai",
        model: "gpt-4o",
        api_key: "sk-vision",
        base_url: "https://api.openai.com/v1",
        capability: "multimodal",
        enabled: true,
      },
    ]);

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A quiet moonlit observatory" } },
    );

    const optimizeButton = screen.getByRole("button", { name: "Optimize" });
    expect(optimizeButton).not.toBeDisabled();
    fireEvent.click(optimizeButton);

    await waitFor(() => {
      expect(optimizePrompt).toHaveBeenCalledWith(
        "A quiet moonlit observatory",
        "vision-helper",
        undefined,
      );
    });
  });

  it("uses prompt agent instead of direct generation in deep thinking mode", async () => {
    getLlmConfigs.mockResolvedValue([
      {
        id: "llm-a",
        name: "Prompt Director",
        protocol: "openai",
        model: "gpt-4.1",
        api_key: "sk-test",
        base_url: "",
        capability: "text",
        enabled: true,
      },
    ]);
    startPromptAgentSession.mockResolvedValueOnce({
      session: {
        id: "agent-session-1",
        status: "active",
        original_prompt: "Design a floating tea house",
        draft_prompt: "A floating tea house above a misty lake",
        accepted_prompt: null,
        selected_skill_ids: ["composition"],
        suggested_params: {},
        created_at: "2026-05-16T00:00:00Z",
        updated_at: "2026-05-16T00:00:00Z",
      },
      messages: [],
    });

    render(<GeneratePage />, { wrapper: TestWrapper });

    fireEvent.click(
      await screen.findByRole("button", { name: "Deep thinking" }),
    );
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "Design a floating tea house" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => expect(startPromptAgentSession).toHaveBeenCalled());
    expect(startPromptAgentSession.mock.calls[0][0]).toEqual({
      prompt: "Design a floating tea house",
      configId: "llm-a",
      conversationId: "conversation-1",
      projectId: "project-1",
      sourceImagePaths: [],
    });
    expect(generateImage).not.toHaveBeenCalled();
  });

  it("prefills the composer with a pending prompt when arriving from prompt extraction", async () => {
    consumePendingPrompt.mockReturnValue("cinematic portrait");

    render(<GeneratePage />, { wrapper: TestWrapper });

    expect(
      await screen.findByDisplayValue("cinematic portrait"),
    ).toBeInTheDocument();
  });

  it("prefills the composer from navigation state when arriving from prompt extraction", async () => {
    useLocation.mockReturnValue({
      state: {
        pendingPrompt: "stateful cinematic portrait",
        activateConversationId: "conversation-new-1",
      },
    });
    consumePendingPrompt.mockReturnValue(null);

    render(<GeneratePage />, { wrapper: TestWrapper });

    expect(
      await screen.findByDisplayValue("stateful cinematic portrait"),
    ).toBeInTheDocument();
  });

  it("saves a sent prompt as a prompt favorite from the message actions", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Favorite prompt" }));

    await waitFor(() => {
      expect(createPromptFavorite).toHaveBeenCalledWith(
        "A dramatic mountain temple at sunrise",
      );
    });
    expect(
      screen.getByRole("button", { name: "Remove prompt favorite" }),
    ).toBeInTheDocument();
  });

  it("hides GPT Image 2 moderation while submitting selected visible generation parameters", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A luminous glass observatory" } },
    );
    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "transparent" },
    });
    fireEvent.change(screen.getByLabelText("Format"), {
      target: { value: "webp" },
    });
    expect(screen.queryByLabelText("Moderation")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Compression")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          prompt: "A luminous glass observatory",
          background: "transparent",
          outputFormat: "webp",
        }),
      );
    });
    expect(generateImage.mock.calls[0][0]).not.toHaveProperty("moderation");
    expect(generateImage.mock.calls[0][0]).not.toHaveProperty(
      "outputCompression",
    );
  });

  it("uses the selected UI model for the next generate request even while model persistence is still in flight", async () => {
    const saveModelDeferred = createDeferred<void>();
    saveImageModel.mockReturnValue(saveModelDeferred.promise);

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "nano-banana" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A neon paper crane" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "A neon paper crane",
        }),
      );
    });
  });

  it("keeps the returned generated image visible when conversation reload data is stale", async () => {
    generateImage.mockResolvedValue({
      generation_id: "generation-new",
      conversation_id: "conversation-1",
      images: [
        {
          id: "image-new",
          generation_id: "generation-new",
          file_path: "/tmp/generated-nano-banana.png",
          thumbnail_path: "/tmp/generated-nano-banana-thumb.png",
        },
      ],
    });

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "nano-banana" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A luminous banana nebula" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "A luminous banana nebula",
        }),
      );
    });
    await waitFor(() => {
      expect(
        screen.getByText("/tmp/generated-nano-banana.png"),
      ).toBeInTheDocument();
    });
  });

  it("ignores a stale conversation load that resolves after the active conversation changes", async () => {
    const firstLoad = createDeferred<unknown[]>();
    getConversationGenerations
      .mockReturnValueOnce(firstLoad.promise)
      .mockResolvedValueOnce([
        {
          generation: {
            id: "generation-current",
            prompt: "Current conversation prompt",
            created_at: "2026-04-26T00:00:00Z",
            status: "completed",
            source_image_paths: [],
          },
          images: [
            {
              id: "image-current",
              generation_id: "generation-current",
              file_path: "/tmp/current-conversation.png",
              thumbnail_path: "/tmp/current-conversation-thumb.png",
            },
          ],
        },
      ]);

    const { rerender } = render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    activeConversationId = "conversation-2";
    rerender(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-2");
    });
    expect(
      await screen.findByText("/tmp/current-conversation.png"),
    ).toBeInTheDocument();

    await act(async () => {
      firstLoad.resolve([
        {
          generation: {
            id: "generation-stale",
            prompt: "Stale conversation prompt",
            created_at: "2026-04-26T00:00:00Z",
            status: "completed",
            source_image_paths: [],
          },
          images: [
            {
              id: "image-stale",
              generation_id: "generation-stale",
              file_path: "/tmp/stale-conversation.png",
              thumbnail_path: "/tmp/stale-conversation-thumb.png",
            },
          ],
        },
      ]);
    });

    expect(
      screen.getByText("/tmp/current-conversation.png"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("/tmp/stale-conversation.png"),
    ).not.toBeInTheDocument();
  });

  it("reloads the active conversation when a generation completion event arrives", async () => {
    let completeHandler:
      ((data: { generation_id: string; status: string }) => void) | undefined;
    onGenerationComplete.mockImplementation((handler) => {
      completeHandler = handler;
      return Promise.resolve(vi.fn());
    });
    getConversationGenerations.mockResolvedValueOnce([]).mockResolvedValueOnce([
      {
        generation: {
          id: "generation-event",
          prompt: "A completed event image",
          created_at: "2026-04-26T00:00:00Z",
          status: "completed",
          source_image_paths: [],
        },
        images: [
          {
            id: "image-event",
            generation_id: "generation-event",
            file_path: "/tmp/event-generated.png",
            thumbnail_path: "/tmp/event-generated-thumb.png",
          },
        ],
      },
    ]);

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(onGenerationComplete).toHaveBeenCalled();
    });

    completeHandler?.({
      generation_id: "generation-event",
      status: "completed",
    });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByText("/tmp/event-generated.png")).toBeInTheDocument();
  });

  it("resets parameters to selected model defaults and hides unsupported controls when switching models", async () => {
    pickSourceImages.mockResolvedValue(["/tmp/source-edit.png"]);
    const { getImageModelCatalogEntry } = await import("../lib/modelCatalog");
    const geminiEntry = getImageModelCatalogEntry("nano-banana");

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    expect(screen.getByLabelText("Size")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Quality"), {
      target: { value: "high" },
    });
    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "transparent" },
    });
    fireEvent.change(screen.getByLabelText("Format"), {
      target: { value: "webp" },
    });
    expect(screen.queryByLabelText("Moderation")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Upload Source" }));

    await waitFor(() => {
      expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    });

    expect(screen.queryByLabelText("Input fidelity")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "nano-banana" },
    });

    await waitFor(() => {
      expect(saveImageModel).toHaveBeenCalledWith("nano-banana");
    });

    expect(screen.queryByLabelText("Size")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Count")).toBeDisabled();
    expect(screen.queryByLabelText("Quality")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Background")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Format")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Moderation")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Input fidelity")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Upload Source" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
    ).toBeInTheDocument();

    fireEvent.change(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
      { target: { value: "A polished chrome crane" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          prompt: "A polished chrome crane",
          model: "nano-banana",
          sourceImagePaths: ["/tmp/source-edit.png"],
          imageCount: geminiEntry.parameterDefaults.imageCount,
        }),
      );
    });
    expect(editImage.mock.calls[0][0]).toHaveProperty(
      "size",
      geminiEntry.parameterDefaults.size,
    );
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("quality");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("background");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("outputFormat");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("moderation");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("inputFidelity");
    expect(generateImage).not.toHaveBeenCalled();
  });

  it("reconciles invalid draft state when late hydration switches to a narrower Gemini model", async () => {
    const initialModelDeferred = createDeferred<"nano-banana">();
    getImageModel.mockReturnValue(initialModelDeferred.promise);
    pickSourceImages.mockResolvedValue(["/tmp/hydration-source.png"]);

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    expect(screen.getByLabelText("Size")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Upload Source" }));

    await waitFor(() => {
      expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    });
    fireEvent.change(screen.getByLabelText("Quality"), {
      target: { value: "high" },
    });
    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "transparent" },
    });
    fireEvent.change(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
      { target: { value: "Turn it into etched silver" } },
    );

    initialModelDeferred.resolve("nano-banana");

    await waitFor(() => {
      expect(getImageModel).toHaveBeenCalled();
    });

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveValue("nano-banana");
    });
    expect(
      screen.getByRole("button", { name: "Upload Source" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByDisplayValue("Turn it into etched silver"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "Turn it into etched silver",
          imageCount: 1,
          sourceImagePaths: ["/tmp/hydration-source.png"],
        }),
      );
    });
    expect(editImage.mock.calls[0][0]).toHaveProperty("size", "1024x1024");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("quality");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("background");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("outputFormat");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("moderation");
    expect(generateImage).not.toHaveBeenCalled();
  });

  it("keeps picked edit sources when source picking resolves after hydration switches to Gemini", async () => {
    const initialModelDeferred = createDeferred<"nano-banana">();
    const pickedSourcesDeferred = createDeferred<string[]>();
    getImageModel.mockReturnValue(initialModelDeferred.promise);
    pickSourceImages.mockReturnValue(pickedSourcesDeferred.promise);

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Upload Source" }));

    initialModelDeferred.resolve("nano-banana");
    pickedSourcesDeferred.resolve(["/tmp/picked-after-hydration.png"]);

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveValue("nano-banana");
    });

    await waitFor(() => {
      expect(pickSourceImages).toHaveBeenCalled();
    });

    expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
    ).toBeInTheDocument();

    fireEvent.change(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
      { target: { value: "A brushed steel heron" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "A brushed steel heron",
          sourceImagePaths: ["/tmp/picked-after-hydration.png"],
        }),
      );
    });
    expect(generateImage).not.toHaveBeenCalled();
  });

  it("uses image-to-edit entry points under a Gemini model", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "nano-banana" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Edit image" }));

    expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
    ).toBeInTheDocument();

    fireEvent.change(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
      { target: { value: "A paper lantern koi" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "A paper lantern koi",
          sourceImagePaths: ["/tmp/source.png"],
        }),
      );
    });
    expect(generateImage).not.toHaveBeenCalled();
  });

  it("prevents transparent backgrounds with jpeg output", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "transparent" },
    });
    expect(screen.getByRole("option", { name: "JPEG" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "auto" },
    });
    fireEvent.change(screen.getByLabelText("Format"), {
      target: { value: "jpeg" },
    });
    expect(screen.getByRole("option", { name: "Transparent" })).toBeDisabled();
  });

  it("keeps common generation parameters in a responsive grid", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    expect(
      screen.getByRole("toolbar", { name: "Generation parameters" }),
    ).toHaveClass("w-full");
    expect(
      screen.getByRole("toolbar", { name: "Generation parameters" }),
    ).not.toHaveClass("overflow-x-auto");
    expect(screen.getByTestId("generation-parameter-row")).toHaveClass(
      "grid",
      "min-w-0",
      "grid-cols-2",
    );
    expect(screen.getByTestId("generation-parameter-row")).not.toHaveClass(
      "w-max",
    );
  });

  it("uses the Studio OS command surface for generation controls", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    const toolbar = screen.getByRole("toolbar", {
      name: "Generation parameters",
    });
    const promptInput = screen.getByPlaceholderText(
      "Describe the image you want to generate...",
    );
    const commandSurface = promptInput.closest(
      "[data-testid='generation-command-surface']",
    );

    expect(document.getElementById("generation-parameters")).toContainElement(
      toolbar,
    );
    expect(commandSurface).not.toContainElement(screen.getByLabelText("Model"));
    expect(promptInput).toHaveClass("studio-input");
  });

  it("presents the composer as a command dock with a compact parameter rail", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    const toolbar = screen.getByRole("toolbar", {
      name: "Generation parameters",
    });
    const commandSurface = screen
      .getByPlaceholderText("Describe the image you want to generate...")
      .closest("[data-testid='generation-command-surface']");

    expect(toolbar).toHaveAttribute("data-variant", "parameter-rail");
    expect(document.getElementById("generation-parameters")).toContainElement(
      toolbar,
    );
  });

  it("lets the generate surface stretch across the available main panel", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    const toolbar = screen.getByRole("toolbar", {
      name: "Generation parameters",
    });
    const feedSurface = screen
      .getByRole("button", { name: "Edit prompt" })
      .closest("[data-testid='generation-feed']");
    const composerSurface = screen.getByPlaceholderText(
      "Describe the image you want to generate...",
    ).parentElement?.parentElement;

    expect(toolbar).toHaveClass("w-full");
    expect(toolbar.className).not.toContain("max-w-[900px]");
    expect(feedSurface).toHaveClass("w-full");
    expect(feedSurface?.className).not.toContain("max-w-[900px]");
    expect(composerSurface).toHaveClass("w-full");
    expect(composerSurface?.className).not.toContain("max-w-[900px]");
  });

  it("keeps the chat feed in a shrinkable internal scroll area", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    const feedSurface = screen
      .getByRole("button", { name: "Edit prompt" })
      .closest("[data-testid='generation-feed']");
    const scrollContainer = feedSurface?.parentElement;

    expect(scrollContainer).toHaveClass("flex-1", "min-h-0", "overflow-y-auto");
  });

  it("hides GPT Image 2 input fidelity while submitting selected source images", async () => {
    pickSourceImages.mockResolvedValue(["/tmp/source-edit.png"]);

    render(<GeneratePage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Upload Source" }));

    await waitFor(() => {
      expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    });

    expect(screen.queryByLabelText("Input fidelity")).not.toBeInTheDocument();
    fireEvent.change(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
      { target: { value: "Make the source image look like a lithograph" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          prompt: "Make the source image look like a lithograph",
          sourceImagePaths: ["/tmp/source-edit.png"],
        }),
      );
    });
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("inputFidelity");
  });

  it("submits latest-model quality and native Grok resolution controls", async () => {
    generateImage
      .mockResolvedValueOnce({
        generation_id: "generation-flare",
        conversation_id: "conversation-1",
        images: [],
      })
      .mockResolvedValueOnce({
        generation_id: "generation-grok",
        conversation_id: "conversation-1",
        images: [],
      });
    render(<GeneratePage />, { wrapper: TestWrapper });
    await waitFor(() =>
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1"),
    );
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "gpt-image-2.5-flare" },
    });
    fireEvent.change(screen.getByLabelText("Quality"), {
      target: { value: "max" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A lighthouse" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "gpt-image-2.5-flare",
          quality: "max",
        }),
      ),
    );
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "grok-imagine-image-2.0" },
    });
    expect(screen.getByLabelText("Quality")).toHaveValue("auto");
    fireEvent.change(screen.getByLabelText("Resolution"), {
      target: { value: "2K" },
    });
    fireEvent.change(screen.getByLabelText("Aspect ratio"), {
      target: { value: "16:9" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "An ocean" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "grok-imagine-image-2.0",
          quality: "auto",
          resolution: "2K",
          aspectRatio: "16:9",
        }),
      ),
    );
  });
  it("submits 2.6 GIF settings from the composer without stale streaming controls", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });
    await waitFor(() => expect(getImageModel).toHaveBeenCalled());
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "gpt-image-2.6-flare" },
    });
    fireEvent.click(screen.getByLabelText("generate.advanced.stream"));
    fireEvent.change(screen.getByLabelText("Format"), {
      target: { value: "gif" },
    });
    fireEvent.change(screen.getByLabelText("generate.advanced.complexity"), {
      target: { value: "low" },
    });
    fireEvent.change(screen.getByLabelText("generate.advanced.frameCount"), {
      target: { value: "12" },
    });
    fireEvent.change(screen.getByLabelText("generate.advanced.frameDuration"), {
      target: { value: "100" },
    });
    fireEvent.change(screen.getByLabelText("Count"), {
      target: { value: "10" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A looping sunrise" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "gpt-image-2.6-flare",
          outputFormat: "gif",
          size: "auto",
          imageCount: 10,
          advancedOptions: {
            complexity: "low",
            frameCount: 12,
            frameDurationMs: 100,
          },
        }),
      ),
    );
    expect(generateImage.mock.calls[0][0]).not.toHaveProperty(
      "outputCompression",
    );
  });
  it("switches SVG to one output and keeps author controls optional", async () => {
    render(<GeneratePage />, { wrapper: TestWrapper });
    await waitFor(() => expect(getImageModel).toHaveBeenCalled());
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "gpt-image-2.6-sunburst" },
    });
    fireEvent.change(screen.getByLabelText("Count"), {
      target: { value: "4" },
    });
    fireEvent.change(screen.getByLabelText("Format"), {
      target: { value: "svg" },
    });
    expect(screen.getByLabelText("Count")).toHaveValue("1");
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A vector logo" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "gpt-image-2.6-sunburst",
          outputFormat: "svg",
          imageCount: 1,
        }),
      ),
    );
    expect(generateImage.mock.calls[0][0].advancedOptions).not.toHaveProperty(
      "svg",
    );
  });
  it("follows growing result content only while the user stays at the latest messages", async () => {
    let resize!: () => void;
    const observe = vi.fn();
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(callback: ResizeObserverCallback) {
          resize = () => callback([], this as unknown as ResizeObserver);
        }
        observe = observe;
        disconnect = vi.fn();
      },
    );
    try {
      render(<GeneratePage />, { wrapper: TestWrapper });
      await screen.findByRole("button", { name: "Edit prompt" });
      const content = screen.getByTestId("generation-feed");
      const scroll = content.parentElement!;
      let height = 900;
      Object.defineProperties(scroll, {
        clientHeight: { configurable: true, value: 300 },
        scrollHeight: { configurable: true, get: () => height },
      });
      scroll.scrollTop = 600;
      fireEvent.scroll(scroll);
      await act(async () => {
        height = 1200;
        resize();
      });
      expect(scroll.scrollTop).toBe(1200);
      scroll.scrollTop = 900;
      fireEvent.scroll(scroll);
      await act(async () => {
        height = 1500;
        resize();
      });
      expect(scroll.scrollTop).toBe(1500);
      expect(observe).toHaveBeenCalledWith(content);
      scroll.scrollTop = 100;
      fireEvent.scroll(scroll);
      await act(async () => {
        height = 1800;
        resize();
      });
      expect(scroll.scrollTop).toBe(100);
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
