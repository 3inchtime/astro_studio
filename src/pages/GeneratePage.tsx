import { useState, useCallback, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  generateImage,
  toAssetUrl,
} from "../lib/api";
import { cn } from "../lib/utils";
import type { ImageSize, ImageQuality, Task } from "../types";
import {
  Loader2,
  CheckCircle2,
  XCircle,
  Image as ImageIcon,
  RefreshCw,
  Copy,
  Maximize2,
  Download,
  ChevronDown,
  Bot,
  ArrowUp,
} from "lucide-react";

const DEFAULT_QUALITY: ImageQuality = "high";
const MAX_TASKS = 50;

const sizes: { value: ImageSize; label: string; desc: string }[] = [
  { value: "1024x1024", label: "1:1", desc: "Square" },
  { value: "1536x1024", label: "3:2", desc: "Landscape" },
  { value: "1024x1536", label: "2:3", desc: "Portrait" },
];

const imageActions = [
  { icon: RefreshCw, label: "Regenerate" },
  { icon: Copy, label: "Variant" },
  { icon: Maximize2, label: "Upscale" },
  { icon: Download, label: "Save" },
];

export default function GeneratePage() {
  const [prompt, setPrompt] = useState("");
  const [size, setSize] = useState<ImageSize>("1024x1024");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [showModelDropdown, setShowModelDropdown] = useState(false);
  const [showSizeDropdown, setShowSizeDropdown] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const updateTask = useCallback(
    (id: string, patch: Partial<Task>) => {
      setTasks((prev) =>
        prev.map((t) => (t.id === id ? { ...t, ...patch } : t)),
      );
    },
    [],
  );

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [tasks, activeTaskId]);

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

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = Math.min(textareaRef.current.scrollHeight, 120) + "px";
    }
  }, [prompt]);

  async function handleGenerate() {
    if (!prompt.trim()) return;

    const tempId = crypto.randomUUID();
    const placeholder: Task = {
      id: tempId,
      prompt,
      size,
      quality: DEFAULT_QUALITY,
      status: "processing",
      imagePath: null,
      error: null,
      createdAt: Date.now(),
    };
    setTasks((prev) => [...prev.slice(-(MAX_TASKS - 1)), placeholder]);
    setActiveTaskId(tempId);
    setPrompt("");

    try {
      const result = await generateImage({ prompt, size, quality: DEFAULT_QUALITY });
      const imagePath = result.image_paths.length > 0 ? result.image_paths[0] : null;

      setTasks((prev) =>
        prev.map((t) =>
          t.id === tempId
            ? { ...t, id: result.generation_id, status: "completed", imagePath }
            : t,
        ),
      );
      setActiveTaskId(result.generation_id);
    } catch (e) {
      updateTask(tempId, { status: "failed", error: String(e) });
    }
  }

  const currentSizeLabel = sizes.find((s) => s.value === size)?.label ?? "1:1";

  return (
    <div className="flex h-full flex-col">
      <div ref={scrollRef} className="flex-1 overflow-y-auto">
        {tasks.length === 0 ? (
          <EmptyState />
        ) : (
          <div className="mx-auto max-w-[720px] space-y-6 px-6 py-6">
            <AnimatePresence initial={false}>
              {tasks.map((task) => (
                <TaskThread
                  key={task.id}
                  task={task}
                  isActive={task.id === activeTaskId}
                  onSelect={() => setActiveTaskId(task.id)}
                />
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>

      {tasks.length > 1 && (
        <div className="flex items-center gap-1 border-t border-border-subtle bg-surface/80 px-6 py-2 overflow-x-auto backdrop-blur-sm">
          {tasks.map((task) => {
            const isActive = task.id === activeTaskId;
            return (
              <motion.button
                key={task.id}
                layout
                onClick={() => setActiveTaskId(task.id)}
                className={cn(
                  "flex shrink-0 items-center gap-1.5 rounded-[8px] px-2.5 py-1 text-[11px] font-medium transition-all duration-200",
                  isActive
                    ? "bg-primary/6 text-primary shadow-card"
                    : "text-muted hover:bg-subtle hover:text-foreground"
                )}
              >
                <TaskStatusIcon status={task.status} />
                <span className="max-w-[90px] truncate">
                  {task.prompt.slice(0, 16)}
                </span>
              </motion.button>
            );
          })}
        </div>
      )}

      <div className="border-t border-border-subtle bg-surface shadow-panel px-6 py-4">
        <div className="mx-auto flex max-w-[720px] items-end gap-2.5">
          <div ref={dropdownRef} className="flex shrink-0 gap-1.5">
            <div className="relative">
              <button
                onClick={() => { setShowModelDropdown(!showModelDropdown); setShowSizeDropdown(false); }}
                className="flex h-[34px] items-center gap-1 rounded-[8px] border border-border-subtle bg-subtle/50 px-2.5 text-[11px] text-muted transition-all duration-150 hover:border-border hover:bg-surface"
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
                className="flex h-[34px] items-center gap-1 rounded-[8px] border border-border-subtle bg-subtle/50 px-2.5 text-[11px] text-muted transition-all duration-150 hover:border-border hover:bg-surface"
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
                        <span className="text-[10px] opacity-50">{s.desc}</span>
                      </button>
                    ))}
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>

          <div className="flex-1 relative">
            <textarea
              ref={textareaRef}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder="描述你想要生成的图像..."
              rows={1}
              className="w-full resize-none rounded-[10px] border border-border-subtle bg-subtle/30 px-4 py-2.5 pr-10 text-[13px] text-foreground placeholder:text-muted/50 focus:outline-none focus:border-primary/30 focus:bg-surface focus:shadow-card transition-all duration-200"
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleGenerate();
                }
              }}
            />
          </div>

          <motion.button
            onClick={handleGenerate}
            disabled={!prompt.trim()}
            whileHover={{ scale: 1.03 }}
            whileTap={{ scale: 0.96 }}
            className="gradient-primary breathe flex h-[38px] shrink-0 items-center gap-2 rounded-[10px] px-5 text-[13px] font-medium text-white shadow-button transition-shadow hover:shadow-float disabled:opacity-30 disabled:pointer-events-none disabled:animate-none"
          >
            <ArrowUp size={16} strokeWidth={2.5} />
          </motion.button>
        </div>
      </div>
    </div>
  );
}

function EmptyState() {
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
          What will you create?
        </p>
        <p className="mt-2 max-w-[260px] text-[13px] leading-relaxed text-muted">
          Describe an image below and press Enter to bring your imagination to life.
        </p>
      </motion.div>
    </div>
  );
}

function TaskThread({
  task,
  isActive,
  onSelect,
}: {
  task: Task;
  isActive: boolean;
  onSelect: () => void;
}) {
  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -10, scale: 0.98 }}
      transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
      onClick={onSelect}
      className={cn(
        "space-y-3 transition-opacity cursor-pointer",
        isActive ? "" : "opacity-50 hover:opacity-75"
      )}
    >
      <div className="flex justify-end">
        <div className="max-w-[75%] rounded-[12px] rounded-br-[4px] bg-bubble px-4 py-2.5">
          <p className="text-[13px] leading-relaxed text-foreground/70">
            {task.prompt}
          </p>
        </div>
      </div>

      <div className="flex items-start gap-2">
        <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-[8px] gradient-primary mt-0.5">
          <Bot size={12} className="text-white" strokeWidth={2.5} />
        </div>
        <div className="flex-1 min-w-0">
          {task.status === "processing" && (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-[12px] text-muted">
                <Loader2 size={13} className="animate-spin text-primary" />
                Generating...
              </div>
              <div className="grid grid-cols-2 gap-2">
                {[0, 1, 2, 3].map((i) => (
                  <div
                    key={i}
                    className="aspect-square rounded-[10px] shimmer"
                    style={{ animationDelay: `${i * 0.15}s` }}
                  />
                ))}
              </div>
            </div>
          )}

          {task.status === "completed" && task.imagePath && (
            <motion.div
              initial={{ opacity: 0, scale: 0.96 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
              className="space-y-2"
            >
              <div className="group relative overflow-hidden rounded-[12px] bg-surface shadow-card">
                <img
                  src={toAssetUrl(task.imagePath)}
                  alt="Generated"
                  className="w-full object-cover transition-transform duration-500 group-hover:scale-[1.02]"
                />
                <div className="absolute inset-0 flex items-end justify-center bg-gradient-to-t from-black/40 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-200 pb-3">
                  <div className="flex items-center gap-1 rounded-[10px] bg-black/50 glass px-1 py-0.5">
                    {imageActions.map(({ icon: Icon, label }) => (
                      <button
                        key={label}
                        onClick={(e) => e.stopPropagation()}
                        title={label}
                        className="flex h-7 w-7 items-center justify-center rounded-[8px] text-white/80 transition-colors hover:bg-white/10 hover:text-white"
                      >
                        <Icon size={14} strokeWidth={1.8} />
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </motion.div>
          )}

          {task.status === "completed" && !task.imagePath && (
            <div className="rounded-[10px] border border-warning/20 bg-warning/4 p-3 text-[12px] text-warning">
              Image generated but file path not found.
            </div>
          )}

          {task.status === "failed" && task.error && (
            <div className="rounded-[10px] border border-error/20 bg-error/4 p-3 text-[12px] text-error">
              {task.error}
            </div>
          )}
        </div>
      </div>
    </motion.div>
  );
}

function TaskStatusIcon({ status }: { status: Task["status"] }) {
  switch (status) {
    case "processing":
      return <Loader2 size={11} className="animate-spin text-primary" />;
    case "completed":
      return <CheckCircle2 size={11} className="text-success" />;
    case "failed":
      return <XCircle size={11} className="text-error" />;
  }
}
