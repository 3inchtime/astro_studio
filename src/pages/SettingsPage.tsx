import { useEffect, useState, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useNavigate } from "react-router-dom";
import {
  getLogs, clearLogs, getLogSettings, saveLogSettings,
  readLogResponseFile, getTrashSettings, saveTrashSettings,
  getFontSize, getImageModel, saveFontSize, saveImageModel,
  getRuntimeLogs, onRuntimeLog, getModelApiKey, getModelEndpointSettings,
  saveModelApiKey, saveModelEndpointSettings,
} from "../lib/api";
import {
  Check, Cpu, Eye, EyeOff, Globe, Key, Languages, SlidersHorizontal,
  FileText, Trash2, ChevronLeft, ChevronRight, Copy, Type, X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  AppFontSize,
  EndpointSettings,
  EndpointMode,
  ImageModel,
  LogEntry,
  LogSettings,
  RuntimeLogEntry,
  TrashSettings,
} from "../types";
import {
  APP_FONT_SIZE_OPTIONS,
  applyAppFontSize,
  getStoredAppFontSize,
} from "../lib/fontSize";
import {
  IMAGE_MODEL_CATALOG,
  getImageModelCatalogEntry,
} from "../lib/modelCatalog";
import {
  LANGUAGE_OPTIONS,
  normalizeLanguage,
  type SupportedLanguage,
} from "../lib/languages";
import ConfirmDialog from "../components/common/ConfirmDialog";

const DEFAULT_MODEL: ImageModel = "gpt-image-2";
const DEFAULT_MODEL_ENTRY = getImageModelCatalogEntry(DEFAULT_MODEL);
const FONT_SIZE_LABEL_KEYS: Record<AppFontSize, string> = {
  small: "settings.fontSizeSmall",
  medium: "settings.fontSizeMedium",
  large: "settings.fontSizeLarge",
};

function defaultBaseUrlForModel(model: ImageModel): string {
  return getImageModelCatalogEntry(model).connectionDefaults.baseUrl;
}

function defaultGenerationUrlForModel(model: ImageModel): string {
  return getImageModelCatalogEntry(model).connectionDefaults.generationUrl;
}

function defaultEditUrlForModel(model: ImageModel): string {
  return getImageModelCatalogEntry(model).connectionDefaults.editUrl;
}

function defaultEndpointSettingsForModel(model: ImageModel): EndpointSettings {
  return {
    mode: "base_url",
    base_url: defaultBaseUrlForModel(model),
    generation_url: defaultGenerationUrlForModel(model),
    edit_url: defaultEditUrlForModel(model),
  };
}

function modelSupportsEdit(model: ImageModel): boolean {
  return getImageModelCatalogEntry(model).supportsEdit;
}

function usesSharedEditEndpoint(model: ImageModel): boolean {
  const { generationUrl, editUrl } = getImageModelCatalogEntry(model).connectionDefaults;

  return generationUrl === editUrl;
}

function formatProviderName(provider: string): string {
  return provider.charAt(0).toUpperCase() + provider.slice(1);
}

function normalizeEndpointSettings(
  model: ImageModel,
  settings: EndpointSettings,
): EndpointSettings {
  const defaults = defaultEndpointSettingsForModel(model);
  const generationUrl = settings.generation_url.trim() || defaults.generation_url;
  const editUrl = !modelSupportsEdit(model) || usesSharedEditEndpoint(model)
    ? generationUrl
    : settings.edit_url.trim() || defaults.edit_url;

  return {
    mode: settings.mode,
    base_url: settings.base_url.trim() || defaults.base_url,
    generation_url: generationUrl,
    edit_url: editUrl,
  };
}

const cardVariants = {
  hidden: { opacity: 0, y: 10, scale: 0.98 },
  visible: (i: number) => ({
    opacity: 1,
    y: 0,
    scale: 1,
    transition: { delay: i * 0.06, duration: 0.4, ease: [0.22, 1, 0.36, 1] as [number, number, number, number] },
  }),
};

const sectionVariants = {
  hidden: { opacity: 0, y: 8 },
  visible: (i: number) => ({
    opacity: 1,
    y: 0,
    transition: { delay: i * 0.08, duration: 0.35, ease: [0.22, 1, 0.36, 1] as [number, number, number, number] },
  }),
};

function maskKey(key: string): string {
  if (key.length <= 8) return "sk-****";
  return key.slice(0, 3) + "..." + key.slice(-4);
}

const LOG_TYPES = ["", "api_request", "api_response", "generation", "system"] as const;
const LOG_LEVELS = ["", "debug", "info", "warn", "error"] as const;

const levelColors: Record<string, string> = {
  debug: "bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400",
  info: "bg-blue-50 text-blue-600 dark:bg-blue-900/30 dark:text-blue-400",
  warn: "bg-orange-50 text-orange-600 dark:bg-orange-900/30 dark:text-orange-400",
  error: "bg-red-50 text-red-600 dark:bg-red-900/30 dark:text-red-400",
};

const typeLabels: Record<string, string> = {
  api_request: "log.apiRequest",
  api_response: "log.apiResponse",
  generation: "log.generation",
  system: "log.system",
};

const levelLabels: Record<string, string> = {
  debug: "log.debug",
  info: "log.info",
  warn: "log.warn",
  error: "log.error",
};

const SETTINGS_TABS = [
  { id: "general", icon: SlidersHorizontal, labelKey: "settings.general" },
  { id: "model", icon: Cpu, labelKey: "settings.modelConfig" },
  { id: "logs", icon: FileText, labelKey: "log.title" },
] as const;

function formatStructuredText(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

function formatRuntimeLogEntry(log: RuntimeLogEntry): string {
  return [
    `[${log.timestamp}] [${log.level.toUpperCase()}] ${log.target}`,
    log.message,
  ].join("\n");
}

function formatRuntimeLogs(logs: RuntimeLogEntry[]): string {
  return logs.map(formatRuntimeLogEntry).join("\n\n");
}

function formatPersistedLog(log: LogEntry, responseContent: string | null): string {
  const lines = [
    `Time: ${log.timestamp}`,
    `Type: ${log.log_type}`,
    `Level: ${log.level.toUpperCase()}`,
  ];

  if (log.generation_id) {
    lines.push(`Generation ID: ${log.generation_id}`);
  }

  lines.push("Message:", log.message);

  if (log.metadata) {
    lines.push("", "Metadata:", formatStructuredText(log.metadata));
  }

  if (responseContent) {
    lines.push("", "Raw Response:", formatStructuredText(responseContent));
  }

  return lines.join("\n");
}

async function copyTextToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const textArea = document.createElement("textarea");
  textArea.value = text;
  textArea.setAttribute("readonly", "");
  textArea.style.position = "fixed";
  textArea.style.opacity = "0";
  document.body.appendChild(textArea);
  textArea.select();
  const copied = document.execCommand("copy");
  textArea.remove();

  if (!copied) {
    throw new Error("Copy failed");
  }
}

function mergeRuntimeLogs(current: RuntimeLogEntry[], incoming: RuntimeLogEntry[]): RuntimeLogEntry[] {
  const bySequence = new Map<number, RuntimeLogEntry>();

  for (const entry of [...current, ...incoming]) {
    bySequence.set(entry.sequence, entry);
  }

  return Array.from(bySequence.values())
    .sort((a, b) => b.sequence - a.sequence)
    .slice(0, 200);
}

export default function SettingsPage() {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<"general" | "model" | "logs">("general");

  // General settings state
  const [apiKey, setApiKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [keySaved, setKeySaved] = useState(false);
  const [endpointMode, setEndpointMode] = useState<EndpointMode>("base_url");
  const [baseUrl, setBaseUrl] = useState(DEFAULT_MODEL_ENTRY.connectionDefaults.baseUrl);
  const [generationUrl, setGenerationUrl] = useState(
    DEFAULT_MODEL_ENTRY.connectionDefaults.generationUrl,
  );
  const [editUrl, setEditUrl] = useState(DEFAULT_MODEL_ENTRY.connectionDefaults.editUrl);
  const [urlSaved, setUrlSaved] = useState(false);
  const [imageModel, setImageModel] = useState<ImageModel>(DEFAULT_MODEL);
  const [modelSaved, setModelSaved] = useState(false);
  const { t, i18n } = useTranslation();
  const [language, setLanguage] = useState<SupportedLanguage>(() =>
    normalizeLanguage(i18n.resolvedLanguage ?? i18n.language),
  );
  const [fontSize, setFontSize] = useState<AppFontSize>(getStoredAppFontSize());
  const [fontSizeSaved, setFontSizeSaved] = useState(false);
  const [trashSettings, setTrashSettings] = useState<TrashSettings>({ retention_days: 30 });
  const [trashSaved, setTrashSaved] = useState(false);

  // Logs state
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [totalLogs, setTotalLogs] = useState(0);
  const [logPage, setLogPage] = useState(1);
  const [logType, setLogType] = useState("");
  const [logLevel, setLogLevel] = useState("");
  const [logSettings, setLogSettings] = useState<LogSettings>({ enabled: true, retention_days: 7 });
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null);
  const [responseContent, setResponseContent] = useState<string | null>(null);
  const [runtimeLogs, setRuntimeLogs] = useState<RuntimeLogEntry[]>([]);
  const [autoScrollRuntimeLogs, setAutoScrollRuntimeLogs] = useState(true);
  const [copiedLogTarget, setCopiedLogTarget] = useState<"runtime" | "detail" | null>(null);
  const [clearLogsOpen, setClearLogsOpen] = useState(false);
  const [clearingLogs, setClearingLogs] = useState(false);
  const runtimeLogsRef = useRef<HTMLDivElement | null>(null);
  const copiedResetRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const imageModelRef = useRef(imageModel);
  const didUserSelectModelRef = useRef(false);

  const pageSize = 20;

  useEffect(() => {
    imageModelRef.current = imageModel;
  }, [imageModel]);

  useEffect(() => {
    let cancelled = false;

    getImageModel().then((model) => {
      if (cancelled || didUserSelectModelRef.current) {
        return;
      }

      setImageModel(model);
    }).catch(() => {
      // Ignore persisted model load failures and keep catalog default.
    });

    getFontSize().then((size) => {
      setFontSize(size);
      applyAppFontSize(size);
    }).catch(() => {
      const storedFontSize = getStoredAppFontSize();
      setFontSize(storedFontSize);
      applyAppFontSize(storedFontSize);
    });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    setApiKey("");
    setShowKey(false);
    setEndpointMode("base_url");
    setBaseUrl(defaultBaseUrlForModel(imageModel));
    setGenerationUrl(defaultGenerationUrlForModel(imageModel));
    setEditUrl(defaultEditUrlForModel(imageModel));

    getModelApiKey(imageModel).then((key) => {
      if (cancelled) {
        return;
      }

      setApiKey(key ?? "");
      setShowKey(false);
    }).catch(() => {
      if (cancelled) {
        return;
      }

      setApiKey("");
      setShowKey(false);
    });

    getModelEndpointSettings(imageModel).then((settings) => {
      if (cancelled) {
        return;
      }

      const normalizedSettings = normalizeEndpointSettings(imageModel, settings);

      setEndpointMode(normalizedSettings.mode);
      setBaseUrl(normalizedSettings.base_url);
      setGenerationUrl(normalizedSettings.generation_url);
      setEditUrl(normalizedSettings.edit_url);
    }).catch(() => {
      if (cancelled) {
        return;
      }

      const defaultSettings = defaultEndpointSettingsForModel(imageModel);

      setEndpointMode(defaultSettings.mode);
      setBaseUrl(defaultSettings.base_url);
      setGenerationUrl(defaultSettings.generation_url);
      setEditUrl(defaultSettings.edit_url);
    });

    return () => {
      cancelled = true;
    };
  }, [imageModel]);

  useEffect(() => {
    getLogSettings().then(setLogSettings);
    getTrashSettings().then(setTrashSettings);
  }, []);

  useEffect(() => {
    setLanguage(normalizeLanguage(i18n.resolvedLanguage ?? i18n.language));
  }, [i18n.language, i18n.resolvedLanguage]);

  const fetchLogs = useCallback(async () => {
    try {
      const result = await getLogs(logType || undefined, logLevel || undefined, logPage, pageSize);
      setLogs(result.logs);
      setTotalLogs(result.total);
    } catch { /* ignore */ }
  }, [logType, logLevel, logPage]);

  useEffect(() => {
    if (activeTab === "logs") fetchLogs();
  }, [activeTab, fetchLogs]);

  useEffect(() => {
    if (activeTab !== "logs") return;

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    onRuntimeLog((entry) => {
      if (cancelled) return;
      setRuntimeLogs((current) => mergeRuntimeLogs(current, [entry]));
    }).then((dispose) => {
      if (cancelled) {
        dispose();
        return;
      }
      unlisten = dispose;
    }).catch(() => {
      unlisten = undefined;
    });

    getRuntimeLogs(200).then((entries) => {
      if (cancelled) return;
      setRuntimeLogs((current) => mergeRuntimeLogs(current, entries));
    }).catch(() => {
      if (!cancelled) setRuntimeLogs([]);
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [activeTab]);

  useEffect(() => {
    if (!autoScrollRuntimeLogs || activeTab !== "logs") return;
    const container = runtimeLogsRef.current;
    if (!container) return;
    container.scrollTop = 0;
  }, [activeTab, autoScrollRuntimeLogs, runtimeLogs]);

  useEffect(() => {
    return () => {
      if (copiedResetRef.current) {
        clearTimeout(copiedResetRef.current);
      }
    };
  }, []);

  function handleLanguageChange(lang: SupportedLanguage) {
    void i18n.changeLanguage(lang);
    setLanguage(lang);
  }

  async function handleSaveKey() {
    const modelAtSaveStart = imageModel;

    await saveModelApiKey(modelAtSaveStart, apiKey);
    if (imageModelRef.current !== modelAtSaveStart) {
      return;
    }

    setShowKey(false);
    setKeySaved(true);
    setTimeout(() => setKeySaved(false), 2000);
  }

  async function handleSaveUrl() {
    const modelAtSaveStart = imageModel;
    const nextBaseUrl = baseUrl.trim() || defaultBaseUrlForModel(imageModel);
    const nextGenerationUrl =
      generationUrl.trim() || defaultGenerationUrlForModel(imageModel);
    const nextEditUrl = !modelSupportsEdit(imageModel) || usesSharedEditEndpoint(imageModel)
      ? nextGenerationUrl
      : editUrl.trim() || defaultEditUrlForModel(imageModel);
    await saveModelEndpointSettings(modelAtSaveStart, {
      mode: endpointMode,
      base_url: nextBaseUrl,
      generation_url: nextGenerationUrl,
      edit_url: nextEditUrl,
    });
    if (imageModelRef.current !== modelAtSaveStart) {
      return;
    }

    setBaseUrl(nextBaseUrl);
    setGenerationUrl(nextGenerationUrl);
    setEditUrl(nextEditUrl);
    setUrlSaved(true);
    setTimeout(() => setUrlSaved(false), 2000);
  }

  async function handleSaveModel() {
    await saveImageModel(imageModel);
    setModelSaved(true);
    setTimeout(() => setModelSaved(false), 2000);
  }

  function handleSelectImageModel(model: ImageModel) {
    didUserSelectModelRef.current = true;
    setImageModel(model);
    setModelSaved(false);
    setKeySaved(false);
    setUrlSaved(false);
  }

  async function handleConfirmClearLogs() {
    setClearingLogs(true);
    try {
      await clearLogs(0);
      setLogs([]);
      setTotalLogs(0);
      setSelectedLog(null);
      setResponseContent(null);
      setLogPage(1);
      setClearLogsOpen(false);
    } catch {
      // Keep the dialog open so the user can retry.
    } finally {
      setClearingLogs(false);
    }
  }

  async function handleSaveRetention(days: number) {
    const newSettings = { ...logSettings, retention_days: days };
    await saveLogSettings(newSettings.enabled, newSettings.retention_days);
    setLogSettings(newSettings);
  }

  async function handleSaveTrashRetention() {
    await saveTrashSettings(trashSettings.retention_days);
    setTrashSaved(true);
    setTimeout(() => setTrashSaved(false), 2000);
  }

  async function handleFontSizeChange(nextSize: AppFontSize) {
    setFontSize(nextSize);
    applyAppFontSize(nextSize);
    await saveFontSize(nextSize);
    setFontSizeSaved(true);
    setTimeout(() => setFontSizeSaved(false), 2000);
  }

  async function handleSelectLog(log: LogEntry) {
    setSelectedLog(log);
    setResponseContent(null);
    if (log.response_file) {
      try {
        const content = await readLogResponseFile(log.response_file);
        setResponseContent(content);
      } catch { /* ignore */ }
    }
  }

  async function handleCopyText(text: string, target: "runtime" | "detail") {
    if (!text.trim()) return;
    await copyTextToClipboard(text);
    setCopiedLogTarget(target);
    if (copiedResetRef.current) {
      clearTimeout(copiedResetRef.current);
    }
    copiedResetRef.current = setTimeout(() => setCopiedLogTarget(null), 1600);
  }

  const totalPages = Math.ceil(totalLogs / pageSize);
  const displayKey = showKey ? apiKey : (apiKey ? maskKey(apiKey) : "");

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto w-full max-w-5xl p-6 md:p-8">
        <motion.h2
          initial={{ opacity: 0, y: -4 }}
          animate={{ opacity: 1, y: 0 }}
          className="mb-7 text-[16px] font-semibold tracking-tight text-foreground"
        >
          {t("settings.title")}
        </motion.h2>

        {/* Tab header */}
        <div className="mb-6 flex gap-1 rounded-[12px] border border-border-subtle bg-subtle/20 p-1">
          {SETTINGS_TABS.map(({ id, icon: Icon, labelKey }) => (
            <button
              key={id}
              onClick={() => setActiveTab(id)}
              className={`relative flex-1 rounded-[10px] px-4 py-2 text-[12px] font-medium transition-all ${
                activeTab === id
                  ? "bg-surface text-foreground shadow-card"
                  : "text-muted/60 hover:text-foreground"
              }`}
            >
              <span className="flex items-center justify-center gap-1.5">
                <Icon size={13} />
                {t(labelKey)}
              </span>
            </button>
          ))}
        </div>

        <AnimatePresence mode="wait">
          {activeTab === "general" ? (
            <motion.div
              key="general"
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -10 }}
              transition={{ duration: 0.2 }}
            >
              <div className="space-y-6">
                {/* General section */}
                <motion.section custom={0} variants={sectionVariants} initial="hidden" animate="visible" className="space-y-3">
                  <div className="flex items-center gap-2">
                    <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[8px] border border-primary/10 bg-primary/5">
                      <SlidersHorizontal size={14} className="text-primary" strokeWidth={2} />
                    </div>
                    <div>
                      <h3 className="text-[13px] font-semibold text-foreground">{t("settings.general")}</h3>
                      <p className="mt-0.5 text-[11px] text-muted/60">{t("settings.generalDesc")}</p>
                    </div>
                  </div>
                  <motion.div custom={0} variants={cardVariants} initial="hidden" animate="visible" className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
                    <div className="grid gap-4 p-5 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-center lg:gap-6">
                      <div className="flex items-start gap-3">
                        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-primary/10 bg-primary/5">
                          <Languages size={14} className="text-primary" strokeWidth={2} />
                        </div>
                        <div>
                          <h4 className="text-[13px] font-semibold text-foreground">{t("settings.language")}</h4>
                          <p className="mt-0.5 text-[11px] leading-relaxed text-muted/60">{t("settings.languageDesc")}</p>
                        </div>
                      </div>
                      <select
                        value={language}
                        onChange={(e) => handleLanguageChange(normalizeLanguage(e.target.value))}
                        className="select-control h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 pr-8 text-[12px] text-foreground transition-all duration-200 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                      >
                        {LANGUAGE_OPTIONS.map((option) => (
                          <option key={option.code} value={option.code}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                    </div>
                  </motion.div>
                  <motion.div custom={1} variants={cardVariants} initial="hidden" animate="visible" className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
                    <div className="grid gap-4 p-5 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-center lg:gap-6">
                      <div className="flex items-start gap-3">
                        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-error/10 bg-error/5">
                          <Trash2 size={14} className="text-error/70" strokeWidth={2} />
                        </div>
                        <div>
                          <h4 className="text-[13px] font-semibold text-foreground">{t("settings.trashRetention")}</h4>
                          <p className="mt-0.5 text-[11px] leading-relaxed text-muted/60">{t("settings.trashRetentionDesc")}</p>
                        </div>
                      </div>
                      <div className="flex min-w-0 flex-col gap-2 lg:flex-row">
                        <div className="flex min-w-0 flex-1 items-center gap-2 rounded-[10px] border border-border-subtle bg-subtle/30 px-3">
                          <input
                            type="number"
                            min={1}
                            max={365}
                            value={trashSettings.retention_days}
                            onChange={(e) => setTrashSettings((prev) => ({
                              ...prev,
                              retention_days: Math.min(365, Math.max(1, Number(e.target.value) || 1)),
                            }))}
                            className="h-[38px] min-w-0 flex-1 bg-transparent text-[12px] text-foreground focus:outline-none"
                          />
                          <span className="text-[11px] text-muted/60">{t("settings.days")}</span>
                        </div>
                        <button
                          onClick={() => void handleSaveTrashRetention()}
                          className="inline-flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] gradient-primary px-4 text-[12px] font-medium text-white shadow-button transition-transform hover:-translate-y-0.5"
                        >
                          {trashSaved && <Check size={13} strokeWidth={2.5} />}
                          {trashSaved ? t("settings.saved") : t("settings.saveTrashRetention")}
                        </button>
                        <button
                          type="button"
                          onClick={() => navigate("/trash")}
                          className="inline-flex h-[38px] shrink-0 items-center justify-center rounded-[10px] border border-border-subtle px-4 text-[12px] font-medium text-foreground/75 transition-all hover:border-border hover:bg-subtle hover:text-foreground"
                        >
                          {t("settings.openTrash")}
                        </button>
                      </div>
                    </div>
                  </motion.div>
                  <motion.div custom={2} variants={cardVariants} initial="hidden" animate="visible" className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
                    <div className="grid gap-4 p-5 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-center lg:gap-6">
                      <div className="flex items-start gap-3">
                        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-primary/10 bg-primary/5">
                          <Type size={14} className="text-primary" strokeWidth={2} />
                        </div>
                        <div>
                          <h4 className="text-[13px] font-semibold text-foreground">{t("settings.fontSize")}</h4>
                          <p className="mt-0.5 text-[11px] leading-relaxed text-muted/60">{t("settings.fontSizeDesc")}</p>
                        </div>
                      </div>
                      <div className="space-y-3">
                        <div className="grid gap-2 sm:grid-cols-3">
                          {APP_FONT_SIZE_OPTIONS.map((option) => {
                            const active = fontSize === option;

                            return (
                              <button
                                key={option}
                                type="button"
                                onClick={() => void handleFontSizeChange(option)}
                                className={`rounded-[10px] border px-3 py-3 text-left transition-all ${
                                  active
                                    ? "border-primary/30 bg-primary/6 shadow-card"
                                    : "border-border-subtle bg-subtle/20 hover:border-border hover:bg-subtle/40"
                                }`}
                              >
                                <div className="flex items-center justify-between gap-3">
                                  <span className="text-[12px] font-medium text-foreground">{t(FONT_SIZE_LABEL_KEYS[option])}</span>
                                  {active && <Check size={13} className="text-primary" strokeWidth={2.5} />}
                                </div>
                                <p className={`mt-2 text-foreground ${option === "small" ? "text-[11px]" : option === "medium" ? "text-[12px]" : "text-[13px]"}`}>
                                  Aa Astro Studio
                                </p>
                              </button>
                            );
                          })}
                        </div>
                        <div className="flex items-center justify-between gap-3 rounded-[10px] border border-border-subtle bg-subtle/20 px-3 py-2.5">
                          <p className="text-[11px] text-muted/60">{t("settings.fontSizePreview")}</p>
                          <span className="text-[11px] font-medium text-primary/80">
                            {fontSizeSaved ? t("settings.saved") : t(FONT_SIZE_LABEL_KEYS[fontSize])}
                          </span>
                        </div>
                      </div>
                    </div>
                  </motion.div>
                </motion.section>

              </div>
            </motion.div>
          ) : activeTab === "model" ? (
            <motion.div
              key="model"
              initial={{ opacity: 0, x: 10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 10 }}
              transition={{ duration: 0.2 }}
            >
              <motion.section custom={0} variants={sectionVariants} initial="hidden" animate="visible" className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[8px] border border-primary/10 bg-primary/5">
                    <Cpu size={14} className="text-primary" strokeWidth={2} />
                  </div>
                  <div>
                    <h3 className="text-[13px] font-semibold text-foreground">{t("settings.modelConfig")}</h3>
                    <p className="mt-0.5 text-[11px] text-muted/60">{t("settings.modelConfigDesc")}</p>
                  </div>
                </div>
                <motion.div custom={0} variants={cardVariants} initial="hidden" animate="visible" className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
                  <div className="grid gap-4 p-5 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-center lg:gap-6">
                    <div className="flex items-start gap-3">
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-primary/10 bg-primary/5">
                        <Cpu size={14} className="text-primary" strokeWidth={2} />
                      </div>
                      <div>
                        <h4 className="text-[13px] font-semibold text-foreground">{t("settings.model")}</h4>
                        <p className="mt-0.5 text-[11px] leading-relaxed text-muted/60">{t("settings.modelDesc")}</p>
                      </div>
                    </div>
                    <div className="min-w-0 space-y-3">
                      <div className="grid gap-2 sm:grid-cols-2">
                        {IMAGE_MODEL_CATALOG.map((entry) => {
                          const active = imageModel === entry.id;

                          return (
                            <button
                              key={entry.id}
                              type="button"
                              aria-pressed={active}
                              aria-label={`Select ${entry.label} model`}
                              onClick={() => handleSelectImageModel(entry.id)}
                              className={`group min-h-[112px] rounded-[10px] border p-3 text-left transition-all ${
                                active
                                  ? "border-primary/35 bg-primary/6 shadow-card"
                                  : "border-border-subtle bg-subtle/20 hover:border-border hover:bg-subtle/35"
                              }`}
                            >
                              <div className="flex items-start justify-between gap-3">
                                <div className="min-w-0">
                                  <div className="flex flex-wrap items-center gap-2">
                                    <span className="text-[13px] font-semibold text-foreground">{entry.label}</span>
                                    <span className="rounded-[6px] border border-border-subtle bg-surface px-1.5 py-0.5 text-[10px] font-medium uppercase text-muted/60">
                                      {formatProviderName(entry.provider)}
                                    </span>
                                  </div>
                                  <p className="mt-1 truncate font-mono text-[10.5px] text-muted/55">
                                    {entry.providerModelId}
                                  </p>
                                </div>
                                <span className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-all ${
                                  active
                                    ? "border-primary bg-primary text-white"
                                    : "border-border-subtle text-transparent group-hover:border-border"
                                }`}>
                                  <Check size={12} strokeWidth={3} />
                                </span>
                              </div>
                              <div className="mt-4 flex flex-wrap gap-1.5">
                                <span className="rounded-[6px] bg-subtle px-2 py-1 text-[10.5px] font-medium text-muted/65">
                                  {entry.supportsEdit ? t("settings.modelSupportsEdit") : t("settings.modelGenerateOnly")}
                                </span>
                                <span className="rounded-[6px] bg-subtle px-2 py-1 text-[10.5px] font-medium text-muted/65">
                                  {entry.connectionDefaults.generationUrl === entry.connectionDefaults.editUrl
                                    ? t("settings.modelSharedEndpoint")
                                    : t("settings.modelSeparateEndpoints")}
                                </span>
                              </div>
                            </button>
                          );
                        })}
                      </div>
                      <div className="flex flex-wrap items-center justify-between gap-3 rounded-[10px] border border-border-subtle bg-subtle/20 px-3 py-2.5">
                        <p className="text-[11px] text-muted/60">
                          {t("settings.selectedModel", {
                            model: getImageModelCatalogEntry(imageModel).label,
                          })}
                        </p>
                        <motion.button
                          type="button"
                          onClick={handleSaveModel}
                          whileTap={{ scale: 0.97 }}
                          className="flex h-[32px] shrink-0 items-center justify-center gap-1.5 rounded-[8px] border border-border-subtle bg-surface px-3 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
                        >
                          {modelSaved ? (<><Check size={13} className="text-success" /><span className="text-success">{t("settings.saved")}</span></>) : t("settings.saveModel")}
                        </motion.button>
                      </div>
                    </div>
                  </div>
                  <div className="border-t border-border-subtle" />
                  <div className="grid gap-4 p-5 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-center lg:gap-6">
                    <div className="flex items-start gap-3">
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-primary/10 bg-primary/5">
                        <Key size={14} className="text-primary" strokeWidth={2} />
                      </div>
                      <div>
                        <h4 className="text-[13px] font-semibold text-foreground">{t("settings.apiKey")}</h4>
                        <p className="mt-0.5 text-[11px] leading-relaxed text-muted/60">{t("settings.apiKeyDesc")}</p>
                      </div>
                    </div>
                    <div className="flex min-w-0 flex-col gap-2 lg:flex-row">
                      <div className="relative min-w-0 flex-1">
                        <input
                          type={showKey ? "text" : "password"}
                          value={displayKey}
                          onChange={(e) => { setApiKey(e.target.value); setKeySaved(false); }}
                          onFocus={() => { if (!showKey) setShowKey(true); }}
                          placeholder={t("settings.apiKeyPlaceholder")}
                          className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 pr-9 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                        />
                        <button
                          type="button"
                          onClick={() => setShowKey(!showKey)}
                          title={showKey ? t("settings.hideKey") : t("settings.showKey")}
                          aria-label={showKey ? t("settings.hideKey") : t("settings.showKey")}
                          className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-muted/40 transition-colors hover:bg-subtle hover:text-muted"
                        >
                          {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
                        </button>
                      </div>
                      <motion.button
                        type="button"
                        onClick={handleSaveKey}
                        disabled={!apiKey.trim()}
                        whileTap={{ scale: 0.97 }}
                        className="flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground disabled:opacity-30 lg:min-w-[104px]"
                      >
                        {keySaved ? (<><Check size={13} className="text-success" /><span className="text-success">{t("settings.saved")}</span></>) : t("settings.saveKey")}
                      </motion.button>
                    </div>
                  </div>
                  <div className="border-t border-border-subtle" />
                  <div className="grid grid-cols-[128px_minmax(0,1fr)] items-start gap-4 p-5 sm:grid-cols-[220px_minmax(0,1fr)] sm:gap-6">
                    <div className="flex items-center gap-3">
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-primary/10 bg-primary/5">
                        <Globe size={14} className="text-primary" strokeWidth={2} />
                      </div>
                      <div>
                        <h4 className="text-[13px] font-semibold text-foreground">{t("settings.endpoint")}</h4>
                      </div>
                    </div>
                    <div className="min-w-0 space-y-3">
                      <div className="grid gap-2 rounded-[10px] border border-border-subtle bg-subtle/20 p-1 sm:grid-cols-2">
                        {(["base_url", "full_url"] as EndpointMode[]).map((mode) => (
                          <button
                            key={mode}
                            type="button"
                            onClick={() => { setEndpointMode(mode); setUrlSaved(false); }}
                            className={`h-[34px] rounded-[8px] px-3 text-[12px] font-medium transition-all ${
                              endpointMode === mode
                                ? "bg-surface text-foreground shadow-card"
                                : "text-muted/60 hover:text-foreground"
                            }`}
                          >
                            {t(mode === "base_url" ? "settings.endpointBaseUrlMode" : "settings.endpointFullUrlMode")}
                          </button>
                        ))}
                      </div>

                      <div className="space-y-1">
                        <p className="text-[11px] leading-relaxed text-muted/60">{t("settings.endpointDesc")}</p>
                        <p className="text-[11px] leading-relaxed text-muted/55">{t("settings.endpointModeHint")}</p>
                      </div>

                      {endpointMode === "base_url" ? (
                        <input
                          type="text"
                          value={baseUrl}
                          onChange={(e) => { setBaseUrl(e.target.value); setUrlSaved(false); }}
                          placeholder={defaultBaseUrlForModel(imageModel)}
                          className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                        />
                      ) : (
                        <div className="grid gap-2">
                          <label className="grid gap-1.5">
                            <span className="text-[11px] font-medium text-muted/70">{t("settings.generationUrl")}</span>
                            <input
                              type="text"
                              value={generationUrl}
                              onChange={(e) => { setGenerationUrl(e.target.value); setUrlSaved(false); }}
                              placeholder={defaultGenerationUrlForModel(imageModel)}
                              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                            />
                          </label>
                          {modelSupportsEdit(imageModel) && !usesSharedEditEndpoint(imageModel) && (
                            <label className="grid gap-1.5">
                              <span className="text-[11px] font-medium text-muted/70">{t("settings.editUrl")}</span>
                              <input
                                type="text"
                                value={editUrl}
                                onChange={(e) => { setEditUrl(e.target.value); setUrlSaved(false); }}
                                placeholder={defaultEditUrlForModel(imageModel)}
                                className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                              />
                            </label>
                          )}
                        </div>
                      )}

                      <div className="flex justify-end">
                        <motion.button
                          type="button"
                          onClick={handleSaveUrl}
                          whileTap={{ scale: 0.97 }}
                          className="flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground lg:min-w-[104px]"
                        >
                          {urlSaved ? (<><Check size={13} className="text-success" /><span className="text-success">{t("settings.saved")}</span></>) : t("settings.saveUrl")}
                        </motion.button>
                      </div>
                    </div>
                  </div>
                </motion.div>
              </motion.section>
            </motion.div>
          ) : (
            <motion.div
              key="logs"
              initial={{ opacity: 0, x: 10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 10 }}
              transition={{ duration: 0.2 }}
            >
              <div className="space-y-4">
                <div className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
                  <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border-subtle px-4 py-3">
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="inline-flex h-2.5 w-2.5 rounded-full bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)]" />
                        <h3 className="text-[13px] font-semibold text-foreground">{t("log.liveTitle")}</h3>
                        <span className="rounded-full border border-emerald-500/20 bg-emerald-500/8 px-2 py-0.5 text-[10px] font-medium text-emerald-600">
                          {t("log.liveConnected")}
                        </span>
                      </div>
                      <p className="mt-1 text-[11px] text-muted/60">{t("log.liveDesc")}</p>
                    </div>
                    <div className="flex items-center gap-2">
                      <label className="flex items-center gap-2 rounded-[8px] border border-border-subtle bg-subtle/20 px-3 py-1.5 text-[11px] text-muted/70">
                        <input
                          type="checkbox"
                          checked={autoScrollRuntimeLogs}
                          onChange={(e) => setAutoScrollRuntimeLogs(e.target.checked)}
                          className="h-3.5 w-3.5 rounded border-border-subtle"
                        />
                        {t("log.autoScroll")}
                      </label>
                      <button
                        type="button"
                        onClick={() => void handleCopyText(formatRuntimeLogs(runtimeLogs), "runtime")}
                        disabled={runtimeLogs.length === 0}
                        className="flex h-[30px] items-center gap-1.5 rounded-[8px] border border-border-subtle px-3 text-[11px] text-muted transition-all hover:border-border hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        <Copy size={12} />
                        {copiedLogTarget === "runtime" ? t("log.copied") : t("log.copyRuntimeLogs")}
                      </button>
                      <button
                        type="button"
                        onClick={() => setRuntimeLogs([])}
                        className="flex h-[30px] items-center gap-1.5 rounded-[8px] border border-border-subtle px-3 text-[11px] text-muted transition-all hover:border-border hover:text-foreground"
                      >
                        <X size={12} />
                        {t("log.clearView")}
                      </button>
                    </div>
                  </div>

                  <div
                    ref={runtimeLogsRef}
                    className="h-[320px] overflow-y-auto bg-[#171412] px-4 py-3 font-mono text-[11px] leading-5 text-stone-200"
                  >
                    {runtimeLogs.length === 0 ? (
                      <div className="flex h-full flex-col items-center justify-center text-center text-stone-400/80">
                        <p className="text-[12px] font-medium">{t("log.liveEmpty")}</p>
                        <p className="mt-1 max-w-md text-[11px]">{t("log.liveHint")}</p>
                      </div>
                    ) : (
                      <div className="space-y-1.5">
                        {runtimeLogs.map((log) => (
                          <div key={log.sequence} className="rounded-[8px] border border-white/5 bg-white/[0.03] px-3 py-2">
                            <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                              <span className="text-stone-500">{log.timestamp}</span>
                              <span className={`rounded-[4px] px-1.5 py-0.5 text-[10px] font-semibold uppercase ${levelColors[log.level] || "bg-stone-700 text-stone-100"}`}>
                                {log.level}
                              </span>
                              <span className="text-stone-400">{log.target}</span>
                            </div>
                            <p className="mt-1 whitespace-pre-wrap break-words text-stone-100">{log.message}</p>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>

                  <div className="flex items-center justify-between border-t border-border-subtle px-4 py-2.5">
                    <span className="text-[11px] text-muted/50">
                      {t("log.liveRecent", { count: runtimeLogs.length })}
                    </span>
                    <span className="text-[11px] text-muted/40">runtime-log:new</span>
                  </div>
                </div>

                {/* Filter bar */}
                <div className="flex flex-wrap items-center gap-3">
                  <select
                    value={logType}
                    onChange={(e) => { setLogType(e.target.value); setLogPage(1); }}
                    className="select-control h-[34px] rounded-[8px] border border-border-subtle bg-subtle/30 px-3 pr-8 text-[12px] text-foreground transition-all focus:border-primary/25 focus:outline-none"
                  >
                    <option value="">{t("log.allTypes")}</option>
                    {LOG_TYPES.filter(Boolean).map((lt) => (
                      <option key={lt} value={lt}>{t(typeLabels[lt] || lt)}</option>
                    ))}
                  </select>

                  <select
                    value={logLevel}
                    onChange={(e) => { setLogLevel(e.target.value); setLogPage(1); }}
                    className="select-control h-[34px] rounded-[8px] border border-border-subtle bg-subtle/30 px-3 pr-8 text-[12px] text-foreground transition-all focus:border-primary/25 focus:outline-none"
                  >
                    <option value="">{t("log.allLevels")}</option>
                    {LOG_LEVELS.filter(Boolean).map((ll) => (
                      <option key={ll} value={ll}>{t(levelLabels[ll] || ll)}</option>
                    ))}
                  </select>

                  <div className="flex items-center gap-2">
                    <span className="text-[11px] text-muted/60">{t("log.retentionDays")}:</span>
                    <select
                      value={logSettings.retention_days}
                      onChange={(e) => handleSaveRetention(Number(e.target.value))}
                      className="select-control h-[30px] rounded-[6px] border border-border-subtle bg-subtle/30 px-2 pr-7 text-[11px] text-foreground focus:border-primary/25 focus:outline-none"
                    >
                      {[3, 7, 14, 30].map((d) => (
                        <option key={d} value={d}>{d} {t("log.days")}</option>
                      ))}
                    </select>
                  </div>

                  <button
                    type="button"
                    onClick={() => setClearLogsOpen(true)}
                    className="ml-auto flex h-[34px] items-center gap-1.5 rounded-[8px] border border-border-subtle px-3 text-[12px] text-muted transition-all hover:border-red-300 hover:text-red-500"
                  >
                    <Trash2 size={12} />
                    {t("log.clearLogs")}
                  </button>
                </div>

                {/* Log list */}
                <div className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
                  {logs.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-16 text-muted/40">
                      <FileText size={32} className="mb-3" />
                      <p className="text-[13px] font-medium">{t("log.noLogs")}</p>
                      <p className="mt-1 text-[11px]">{t("log.noLogsHint")}</p>
                    </div>
                  ) : (
                    <>
                      <div className="grid grid-cols-[140px_100px_70px_1fr] gap-2 border-b border-border-subtle px-4 py-2 text-[11px] font-medium text-muted/50">
                        <span>Time</span>
                        <span>{t("log.filterType")}</span>
                        <span>{t("log.filterLevel")}</span>
                        <span>Message</span>
                      </div>
                      <div className="max-h-[500px] overflow-y-auto">
                        {logs.map((log) => (
                          <button
                            key={log.id}
                            onClick={() => handleSelectLog(log)}
                            className={`grid w-full grid-cols-[140px_100px_70px_1fr] gap-2 border-b border-border-subtle/50 px-4 py-2.5 text-left text-[12px] transition-colors hover:bg-subtle/30 ${
                              selectedLog?.id === log.id ? "bg-primary/5" : ""
                            }`}
                          >
                            <span className="text-muted/60 font-mono text-[11px]">{log.timestamp}</span>
                            <span className="text-muted/70">{log.log_type.replace("_", " ")}</span>
                            <span>
                              <span className={`inline-block rounded-[4px] px-1.5 py-0.5 text-[10px] font-medium ${levelColors[log.level] || ""}`}>
                                {log.level.toUpperCase()}
                              </span>
                            </span>
                            <span className="truncate text-foreground">{log.message}</span>
                          </button>
                        ))}
                      </div>

                      {/* Pagination */}
                      <div className="flex items-center justify-between border-t border-border-subtle px-4 py-2.5">
                        <span className="text-[11px] text-muted/50">
                          {t("log.totalCount", { count: totalLogs })}
                        </span>
                        <div className="flex items-center gap-1">
                          <button
                            disabled={logPage <= 1}
                            onClick={() => setLogPage((p) => p - 1)}
                            className="flex h-7 w-7 items-center justify-center rounded-[6px] text-muted/50 transition-colors hover:bg-subtle hover:text-foreground disabled:opacity-30"
                          >
                            <ChevronLeft size={14} />
                          </button>
                          <span className="px-2 text-[11px] text-muted/60">{logPage} / {totalPages || 1}</span>
                          <button
                            disabled={logPage >= totalPages}
                            onClick={() => setLogPage((p) => p + 1)}
                            className="flex h-7 w-7 items-center justify-center rounded-[6px] text-muted/50 transition-colors hover:bg-subtle hover:text-foreground disabled:opacity-30"
                          >
                            <ChevronRight size={14} />
                          </button>
                        </div>
                      </div>
                    </>
                  )}
                </div>

                {/* Log detail panel */}
                <AnimatePresence>
                  {selectedLog && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: "auto" }}
                      exit={{ opacity: 0, height: 0 }}
                      className="overflow-hidden"
                    >
                      <div className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
                        <div className="flex items-center justify-between border-b border-border-subtle px-4 py-2.5">
                          <span className="text-[12px] font-medium text-foreground">{t("log.detail")}</span>
                          <div className="flex items-center gap-1.5">
                            <button
                              type="button"
                              onClick={() => void handleCopyText(formatPersistedLog(selectedLog, responseContent), "detail")}
                              className="flex h-7 items-center gap-1.5 rounded-[6px] border border-border-subtle px-2 text-[11px] text-muted transition-colors hover:border-border hover:text-foreground"
                            >
                              <Copy size={12} />
                              {copiedLogTarget === "detail" ? t("log.copied") : t("log.copyLog")}
                            </button>
                            <button
                              onClick={() => setSelectedLog(null)}
                              className="flex h-6 w-6 items-center justify-center rounded-[6px] text-muted/40 hover:bg-subtle hover:text-muted"
                            >
                              <X size={13} />
                            </button>
                          </div>
                        </div>
                        <div className="space-y-3 p-4">
                          <div className="grid grid-cols-[80px_1fr] gap-2 text-[12px]">
                            <span className="text-muted/50">Time</span>
                            <span className="font-mono text-foreground">{selectedLog.timestamp}</span>
                            <span className="text-muted/50">Type</span>
                            <span className="text-foreground">{selectedLog.log_type}</span>
                            <span className="text-muted/50">Level</span>
                            <span>
                              <span className={`inline-block rounded-[4px] px-1.5 py-0.5 text-[10px] font-medium ${levelColors[selectedLog.level] || ""}`}>
                                {selectedLog.level.toUpperCase()}
                              </span>
                            </span>
                            <span className="text-muted/50">Message</span>
                            <pre className="whitespace-pre-wrap break-words text-foreground">
                              {selectedLog.message}
                            </pre>
                            {selectedLog.generation_id && (
                              <>
                                <span className="text-muted/50">Gen ID</span>
                                <span className="font-mono text-[11px] text-foreground">{selectedLog.generation_id}</span>
                              </>
                            )}
                          </div>

                          {selectedLog.metadata && (
                            <div>
                              <h4 className="mb-1 text-[11px] font-medium text-muted/60">{t("log.metadata")}</h4>
                              <pre className="max-h-[200px] overflow-auto whitespace-pre-wrap break-words rounded-[8px] bg-subtle/30 p-3 text-[11px] text-foreground">
                                {formatStructuredText(selectedLog.metadata)}
                              </pre>
                            </div>
                          )}

                          {responseContent && (
                            <div>
                              <h4 className="mb-1 text-[11px] font-medium text-muted/60">{t("log.rawResponse")}</h4>
                              <pre className="max-h-[300px] overflow-auto whitespace-pre-wrap break-words rounded-[8px] bg-subtle/30 p-3 text-[11px] text-foreground">
                                {formatStructuredText(responseContent)}
                              </pre>
                            </div>
                          )}
                        </div>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
      <ConfirmDialog
        open={clearLogsOpen}
        title={t("log.clearConfirm")}
        confirmLabel={t("log.clearLogs")}
        cancelLabel={t("favorites.cancel")}
        onConfirm={() => void handleConfirmClearLogs()}
        onCancel={() => setClearLogsOpen(false)}
        loading={clearingLogs}
      />
    </div>
  );
}
