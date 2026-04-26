import { useEffect, useState, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useNavigate } from "react-router-dom";
import {
  saveApiKey, getApiKey, saveBaseUrl, getBaseUrl,
  getLogs, clearLogs, getLogSettings, saveLogSettings,
  readLogResponseFile, getTrashSettings, saveTrashSettings,
  getFontSize, getImageModel, saveFontSize, saveImageModel,
} from "../lib/api";
import {
  Check, Cpu, Eye, EyeOff, Globe, Key, Languages, SlidersHorizontal,
  FileText, Trash2, ChevronLeft, ChevronRight, Type, X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AppFontSize, ImageModel, LogEntry, LogSettings, TrashSettings } from "../types";
import {
  APP_FONT_SIZE_OPTIONS,
  applyAppFontSize,
  getStoredAppFontSize,
} from "../lib/fontSize";

const DEFAULT_BASE_URL = "https://api.openai.com/v1";
const FONT_SIZE_LABEL_KEYS: Record<AppFontSize, string> = {
  small: "settings.fontSizeSmall",
  medium: "settings.fontSizeMedium",
  large: "settings.fontSizeLarge",
};
const IMAGE_MODEL_OPTIONS: ImageModel[] = ["gpt-image-2"];

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

export default function SettingsPage() {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<"general" | "model" | "logs">("general");

  // General settings state
  const [apiKey, setApiKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [keySaved, setKeySaved] = useState(false);
  const [baseUrl, setBaseUrl] = useState(DEFAULT_BASE_URL);
  const [urlSaved, setUrlSaved] = useState(false);
  const [imageModel, setImageModel] = useState<ImageModel>("gpt-image-2");
  const [modelSaved, setModelSaved] = useState(false);
  const { t, i18n } = useTranslation();
  const [language, setLanguage] = useState(i18n.language);
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

  const pageSize = 20;

  useEffect(() => {
    getApiKey().then((key) => { if (key) setApiKey(key); });
    getBaseUrl().then((url) => setBaseUrl(url));
    getImageModel().then((model) => setImageModel(model));
    getFontSize().then((size) => {
      setFontSize(size);
      applyAppFontSize(size);
    }).catch(() => {
      const storedFontSize = getStoredAppFontSize();
      setFontSize(storedFontSize);
      applyAppFontSize(storedFontSize);
    });
  }, []);

  useEffect(() => {
    getLogSettings().then(setLogSettings);
    getTrashSettings().then(setTrashSettings);
  }, []);

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

  function handleLanguageChange(lang: string) {
    i18n.changeLanguage(lang);
    setLanguage(lang);
  }

  async function handleSaveKey() {
    await saveApiKey(apiKey);
    setShowKey(false);
    setKeySaved(true);
    setTimeout(() => setKeySaved(false), 2000);
  }

  async function handleSaveUrl() {
    const url = baseUrl.trim() || DEFAULT_BASE_URL;
    await saveBaseUrl(url);
    setBaseUrl(url);
    setUrlSaved(true);
    setTimeout(() => setUrlSaved(false), 2000);
  }

  async function handleSaveModel() {
    await saveImageModel(imageModel);
    setModelSaved(true);
    setTimeout(() => setModelSaved(false), 2000);
  }

  async function handleClearLogs() {
    if (!confirm(t("log.clearConfirm"))) return;
    await clearLogs();
    setLogPage(1);
    fetchLogs();
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
                        value={language.startsWith("zh") ? "zh-CN" : "en"}
                        onChange={(e) => handleLanguageChange(e.target.value)}
                        className="h-[38px] w-full appearance-none rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                      >
                        <option value="en">English</option>
                        <option value="zh-CN">简体中文</option>
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
                    <div className="flex min-w-0 flex-col gap-2 lg:flex-row">
                      <select
                        value={imageModel}
                        onChange={(e) => {
                          setImageModel(e.target.value as ImageModel);
                          setModelSaved(false);
                        }}
                        className="h-[38px] w-full appearance-none rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                      >
                        {IMAGE_MODEL_OPTIONS.map((model) => (
                          <option key={model} value={model}>{model}</option>
                        ))}
                      </select>
                      <motion.button
                        type="button"
                        onClick={handleSaveModel}
                        whileTap={{ scale: 0.97 }}
                        className="flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground lg:min-w-[104px]"
                      >
                        {modelSaved ? (<><Check size={13} className="text-success" /><span className="text-success">{t("settings.saved")}</span></>) : t("settings.saveModel")}
                      </motion.button>
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
                  <div className="grid gap-4 p-5 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-center lg:gap-6">
                    <div className="flex items-start gap-3">
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-primary/10 bg-primary/5">
                        <Globe size={14} className="text-primary" strokeWidth={2} />
                      </div>
                      <div>
                        <h4 className="text-[13px] font-semibold text-foreground">{t("settings.endpoint")}</h4>
                        <p className="mt-0.5 text-[11px] leading-relaxed text-muted/60">{t("settings.endpointDesc")}</p>
                      </div>
                    </div>
                    <div className="flex min-w-0 flex-col gap-2 lg:flex-row">
                      <input
                        type="text"
                        value={baseUrl}
                        onChange={(e) => { setBaseUrl(e.target.value); setUrlSaved(false); }}
                        placeholder={DEFAULT_BASE_URL}
                        className="h-[38px] min-w-0 flex-1 rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                      />
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
                {/* Filter bar */}
                <div className="flex flex-wrap items-center gap-3">
                  <select
                    value={logType}
                    onChange={(e) => { setLogType(e.target.value); setLogPage(1); }}
                    className="h-[34px] appearance-none rounded-[8px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all focus:border-primary/25 focus:outline-none"
                  >
                    <option value="">{t("log.allTypes")}</option>
                    {LOG_TYPES.filter(Boolean).map((lt) => (
                      <option key={lt} value={lt}>{t(typeLabels[lt] || lt)}</option>
                    ))}
                  </select>

                  <select
                    value={logLevel}
                    onChange={(e) => { setLogLevel(e.target.value); setLogPage(1); }}
                    className="h-[34px] appearance-none rounded-[8px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all focus:border-primary/25 focus:outline-none"
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
                      className="h-[30px] appearance-none rounded-[6px] border border-border-subtle bg-subtle/30 px-2 text-[11px] text-foreground focus:border-primary/25 focus:outline-none"
                    >
                      {[3, 7, 14, 30].map((d) => (
                        <option key={d} value={d}>{d} {t("log.days")}</option>
                      ))}
                    </select>
                  </div>

                  <button
                    onClick={handleClearLogs}
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
                          <button
                            onClick={() => setSelectedLog(null)}
                            className="flex h-6 w-6 items-center justify-center rounded-[6px] text-muted/40 hover:bg-subtle hover:text-muted"
                          >
                            <X size={13} />
                          </button>
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
    </div>
  );
}
