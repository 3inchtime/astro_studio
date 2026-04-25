import { useState, useCallback, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { generateImage, getConversationGenerations, deleteGeneration } from "../lib/api";
import { cn } from "../lib/utils";
import { useLayoutContext } from "../components/layout/AppLayout";
import MessageBubble from "../components/generate/MessageBubble";
import ConversationTab from "../components/generate/ConversationTab";
import Lightbox from "../components/lightbox/Lightbox";
import type { ImageSize, ImageQuality, Message, GenerationResult } from "../types";
import { useTranslation } from "react-i18next";
import {
  Image as ImageIcon,
  ChevronDown,
  ArrowUp,
} from "lucide-react";

const DEFAULT_QUALITY: ImageQuality = "high";

const sizes: { value: ImageSize; label: string; descKey: string }[] = [
  { value: "1024x1024", label: "1:1", descKey: "generate.square" },
  { value: "1536x1024", label: "3:2", descKey: "generate.landscape" },
  { value: "1024x1536", label: "2:3", descKey: "generate.portrait" },
];

interface OpenTab {
  id: string;
  title: string;
}

function generationsToMessages(generations: GenerationResult[]): Message[] {
  const messages: Message[] = [];
  for (const gr of generations) {
    messages.push({
      id: `user-${gr.generation.id}`,
      role: "user",
      content: gr.generation.prompt,
      status: "complete",
      createdAt: gr.generation.created_at,
    });
    const img = gr.images[0];
    messages.push({
      id: `assistant-${gr.generation.id}`,
      role: "assistant",
      content: "",
      generationId: gr.generation.id,
      imagePath: img?.file_path,
      thumbnailPath: img?.thumbnail_path,
      status: gr.generation.status === "completed" ? "complete"
        : gr.generation.status === "failed" ? "failed"
        : "processing",
      createdAt: gr.generation.created_at,
    });
  }
  return messages;
}

export default function GeneratePage() {
  const { t } = useTranslation();
  const { activeConversationId, setActiveConversationId } = useLayoutContext();
  const [messages, setMessages] = useState<Message[]>([]);
  const [prompt, setPrompt] = useState("");
  const [size, setSize] = useState<ImageSize>("1024x1024");
  const [tabs, setTabs] = useState<OpenTab[]>([]);
  const [showModelDropdown, setShowModelDropdown] = useState(false);
  const [showSizeDropdown, setShowSizeDropdown] = useState(false);
  const [lightboxState, setLightboxState] = useState<{
    images: string[];
    index: number;
    imageId: string;
  } | null>(null);

  const scrollRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const autoScrollRef = useRef(true);

  useEffect(() => {
    if (!activeConversationId) {
      setMessages([]);
      return;
    }
    getConversationGenerations(activeConversationId).then((gens) => {
      setMessages(generationsToMessages(gens));
    }).catch(() => {});
  }, [activeConversationId]);

  useEffect(() => {
    if (autoScrollRef.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    autoScrollRef.current = scrollHeight - scrollTop - clientHeight < 100;
  }, []);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = Math.min(textareaRef.current.scrollHeight, 120) + "px";
    }
  }, [prompt]);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setShowModelDropdown(false);
        setShowSizeDropdown(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const handleNewConversation = useCallback(() => {
    setActiveConversationId(null);
    setMessages([]);
    setPrompt("");
  }, [setActiveConversationId]);

  // Add tab when active conversation changes (e.g. from sidebar click)
  useEffect(() => {
    if (activeConversationId) {
      setTabs((prev) => {
        if (prev.some((tab) => tab.id === activeConversationId)) return prev;
        // Load conversation generations to get the title from the first prompt
        getConversationGenerations(activeConversationId).then((gens) => {
          if (gens.length > 0) {
            const firstPrompt = gens[0].generation.prompt;
            const title = firstPrompt.length > 20 ? firstPrompt.slice(0, 20) + "..." : firstPrompt;
            setTabs((prev2) => {
              const exists = prev2.some((tab) => tab.id === activeConversationId);
              if (exists) return prev2;
              return [...prev2, { id: activeConversationId, title }];
            });
          } else {
            setTabs((prev2) => {
              const exists = prev2.some((tab) => tab.id === activeConversationId);
              if (exists) return prev2;
              return [...prev2, { id: activeConversationId, title: "New" }];
            });
          }
        }).catch(() => {});
        // Add placeholder immediately
        const placeholder = prev.some((tab) => tab.id === activeConversationId);
        if (placeholder) return prev;
        return [...prev, { id: activeConversationId, title: "..." }];
      });
    }
  }, [activeConversationId]);

  const closeTab = useCallback((id: string) => {
    setTabs((prev) => {
      const next = prev.filter((t) => t.id !== id);
      if (activeConversationId === id && next.length > 0) {
        setActiveConversationId(next[next.length - 1].id);
      } else if (next.length === 0) {
        setActiveConversationId(null);
      }
      return next;
    });
  }, [activeConversationId, setActiveConversationId]);

  async function handleGenerate() {
    if (!prompt.trim()) return;

    const tempId = crypto.randomUUID();
    const userMsg: Message = {
      id: `user-${tempId}`,
      role: "user",
      content: prompt,
      status: "complete",
      createdAt: new Date().toISOString(),
    };
    const assistantMsg: Message = {
      id: `assistant-${tempId}`,
      role: "assistant",
      content: "",
      status: "processing",
      createdAt: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, userMsg, assistantMsg]);
    autoScrollRef.current = true;
    setPrompt("");

    try {
      const result = await generateImage({ prompt, size, quality: DEFAULT_QUALITY });
      const imagePath = result.image_paths[0] || undefined;
      setMessages((prev) =>
        prev.map((m) =>
          m.id === `assistant-${tempId}`
            ? { ...m, id: `assistant-${result.generation_id}`, generationId: result.generation_id, imagePath, status: "complete" as const }
            : m
        ),
      );
      // Add tab for the new conversation
      const tabTitle = prompt.length > 20 ? prompt.slice(0, 20) + "..." : prompt;
      setTabs((prev) => {
        if (prev.some((tab) => tab.id === result.conversation_id)) return prev;
        return [...prev, { id: result.conversation_id, title: tabTitle }];
      });
      setActiveConversationId(result.conversation_id);
    } catch (e) {
      setMessages((prev) =>
        prev.map((m) =>
          m.id === `assistant-${tempId}`
            ? { ...m, status: "failed" as const, error: String(e) }
            : m
        ),
      );
    }
  }

  const handleImageClick = useCallback((_imagePath: string, allImages: string[], index: number, imageId: string) => {
    setLightboxState({ images: allImages, index, imageId });
  }, []);

  const handleDeleteFromBubble = useCallback(async (generationId: string) => {
    await deleteGeneration(generationId);
    setMessages((prev) => prev.filter((m) => m.generationId !== generationId));
  }, []);

  const currentSizeLabel = sizes.find((s) => s.value === size)?.label ?? "1:1";

  return (
    <div className="flex h-full flex-col">
      <ConversationTab tabs={tabs} activeId={activeConversationId} onSelect={(id) => setActiveConversationId(id)} onClose={closeTab} onNew={handleNewConversation} />

      <div ref={scrollRef} onScroll={handleScroll} className="flex-1 overflow-y-auto">
        {messages.length === 0 ? (
          <EmptyState />
        ) : (
          <div className="mx-auto max-w-[900px] space-y-7 px-6 py-6">
            <AnimatePresence initial={false}>
              {messages.map((msg) => (
                <MessageBubble key={msg.id} message={msg} onImageClick={handleImageClick} onDelete={handleDeleteFromBubble} />
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>

      {/* Settings bar */}
      <div ref={dropdownRef} className="flex items-center gap-4 border-t border-border-subtle bg-subtle/30 px-6 py-2.5">
        <div className="mx-auto flex w-full max-w-[900px] items-center gap-4">
          <div className="relative">
            <button
              onClick={() => { setShowModelDropdown(!showModelDropdown); setShowSizeDropdown(false); }}
              className="flex items-center gap-1 rounded-[8px] border border-border-subtle bg-surface px-2.5 py-1.5 text-[12px] text-foreground transition-all hover:border-border"
            >
              Astro v2
              <ChevronDown size={10} className="opacity-50" />
            </button>
            <AnimatePresence>
              {showModelDropdown && (
                <motion.div
                  initial={{ opacity: 0, y: -3, scale: 0.98 }}
                  animate={{ opacity: 1, y: 0, scale: 1 }}
                  exit={{ opacity: 0, y: -3, scale: 0.98 }}
                  transition={{ duration: 0.12 }}
                  className="absolute bottom-full left-0 mb-1.5 w-40 rounded-[10px] border border-border bg-surface shadow-float py-1 z-50"
                >
                  {["Astro v2.0", "GPT Image"].map((name, i) => (
                    <button
                      key={name}
                      onClick={() => setShowModelDropdown(false)}
                      className={cn(
                        "flex w-full items-center gap-2 px-3 py-2 text-[12px] transition-colors",
                        i === 0 ? "text-primary bg-primary/4" : "text-muted hover:bg-subtle"
                      )}
                    >
                      <span className={cn("h-1.5 w-1.5 rounded-full", i === 0 ? "bg-primary" : "bg-border")} />
                      {name}
                    </button>
                  ))}
                </motion.div>
              )}
            </AnimatePresence>
          </div>

          <div className="relative">
            <button
              onClick={() => { setShowSizeDropdown(!showSizeDropdown); setShowModelDropdown(false); }}
              className="flex items-center gap-1 rounded-[8px] border border-border-subtle bg-surface px-2.5 py-1.5 text-[12px] text-foreground transition-all hover:border-border"
            >
              {currentSizeLabel}
              <ChevronDown size={10} className="opacity-50" />
            </button>
            <AnimatePresence>
              {showSizeDropdown && (
                <motion.div
                  initial={{ opacity: 0, y: -3, scale: 0.98 }}
                  animate={{ opacity: 1, y: 0, scale: 1 }}
                  exit={{ opacity: 0, y: -3, scale: 0.98 }}
                  transition={{ duration: 0.12 }}
                  className="absolute bottom-full left-0 mb-1.5 w-36 rounded-[10px] border border-border bg-surface shadow-float py-1 z-50"
                >
                  {sizes.map((s) => (
                    <button
                      key={s.value}
                      onClick={() => { setSize(s.value); setShowSizeDropdown(false); }}
                      className={cn(
                        "flex w-full items-center justify-between px-3 py-2 text-[12px] transition-colors",
                        size === s.value ? "text-primary bg-primary/4" : "text-muted hover:bg-subtle"
                      )}
                    >
                      <span>{s.label}</span>
                      <span className="text-[10px] opacity-50">{t(s.descKey)}</span>
                    </button>
                  ))}
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      </div>

      {/* Input area */}
      <div className="bg-surface px-6 pt-4 pb-5">
        <div className="mx-auto max-w-[900px]">
          <div className="relative rounded-[18px] border border-border-subtle bg-subtle/40 p-3 transition-all duration-200 focus-within:border-primary/40 focus-within:bg-surface focus-within:shadow-[0_0_0_4px_rgba(79,106,255,0.1)]">
            <textarea
              ref={textareaRef}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder={t("generate.placeholder")}
              rows={2}
              className="w-full resize-none border-none bg-transparent text-[14px] leading-[1.6] text-foreground placeholder:text-muted/50 focus:outline-none pr-[110px]"
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleGenerate();
                }
              }}
            />
            <motion.button
              onClick={handleGenerate}
              disabled={!prompt.trim()}
              whileHover={{ scale: 1.02, y: -1 }}
              whileTap={{ scale: 0.97 }}
              className="absolute right-3 bottom-3 flex items-center gap-2 rounded-[12px] gradient-primary px-5 py-2.5 text-[13px] font-semibold text-white shadow-[0_4px_12px_rgba(79,106,255,0.3)] transition-shadow hover:shadow-[0_6px_16px_rgba(79,106,255,0.4)] disabled:opacity-40 disabled:pointer-events-none disabled:shadow-none"
            >
              <ArrowUp size={15} strokeWidth={2.5} />
            </motion.button>
          </div>
        </div>
      </div>

      {lightboxState && (
        <Lightbox
          images={lightboxState.images}
          initialIndex={lightboxState.index}
          onClose={() => setLightboxState(null)}
          imageId={lightboxState.imageId}
          onDelete={async (_imagePath) => {
            if (!lightboxState.imageId) return;
            const generationId = lightboxState.imageId.replace(/_0$/, "");
            await deleteGeneration(generationId);
            setMessages((prev) => prev.filter((m) => m.generationId !== generationId));
            setLightboxState(null);
          }}
        />
      )}
    </div>
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
        <p className="text-[15px] font-semibold text-foreground tracking-tight">{t("generate.emptyTitle")}</p>
        <p className="mt-2 max-w-[260px] text-[13px] leading-relaxed text-muted">
          {t("generate.emptySubtitle")}
        </p>
      </motion.div>
    </div>
  );
}
