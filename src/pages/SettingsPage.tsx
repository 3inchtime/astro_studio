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
  Cpu, FileText, SlidersHorizontal,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  AppFontSize,
  EndpointMode,
  ImageModel,
  LogEntry,
  LogSettings,
  RuntimeLogEntry,
  TrashSettings,
} from "../types";
import {
  applyAppFontSize,
  getStoredAppFontSize,
} from "../lib/fontSize";
import { getImageModelCatalogEntry } from "../lib/modelCatalog";
import { normalizeLanguage, type SupportedLanguage } from "../lib/languages";
import ConfirmDialog from "../components/common/ConfirmDialog";
import { GeneralSettingsPanel } from "../components/settings/GeneralSettingsPanel";
import { LogsPanel } from "../components/settings/LogsPanel";
import { ModelSettingsPanel } from "../components/settings/ModelSettingsPanel";
import {
  copyTextToClipboard,
  mergeRuntimeLogs,
} from "../lib/settingsLogs";
import {
  defaultBaseUrlForModel,
  defaultEditUrlForModel,
  defaultEndpointSettingsForModel,
  defaultGenerationUrlForModel,
  modelSupportsEdit,
  normalizeEndpointSettings,
  usesSharedEditEndpoint,
} from "../lib/settingsEndpoints";

const DEFAULT_MODEL: ImageModel = "gpt-image-2";
const DEFAULT_MODEL_ENTRY = getImageModelCatalogEntry(DEFAULT_MODEL);
const FONT_SIZE_LABEL_KEYS: Record<AppFontSize, string> = {
  small: "settings.fontSizeSmall",
  medium: "settings.fontSizeMedium",
  large: "settings.fontSizeLarge",
};

function maskKey(key: string): string {
  if (key.length <= 8) return "sk-****";
  return key.slice(0, 3) + "..." + key.slice(-4);
}

const SETTINGS_TABS = [
  { id: "general", icon: SlidersHorizontal, labelKey: "settings.general" },
  { id: "model", icon: Cpu, labelKey: "settings.modelConfig" },
  { id: "logs", icon: FileText, labelKey: "log.title" },
] as const;

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
            <GeneralSettingsPanel
              t={t}
              language={language}
              trashSettings={trashSettings}
              trashSaved={trashSaved}
              fontSize={fontSize}
              fontSizeSaved={fontSizeSaved}
              fontSizeLabelKeys={FONT_SIZE_LABEL_KEYS}
              onLanguageChange={handleLanguageChange}
              onTrashSettingsChange={setTrashSettings}
              onSaveTrashRetention={() => void handleSaveTrashRetention()}
              onOpenTrash={() => navigate("/trash")}
              onFontSizeChange={(nextSize) => void handleFontSizeChange(nextSize)}
            />
          ) : activeTab === "model" ? (
            <ModelSettingsPanel
              t={t}
              imageModel={imageModel}
              modelSaved={modelSaved}
              apiKey={apiKey}
              displayKey={displayKey}
              showKey={showKey}
              keySaved={keySaved}
              endpointMode={endpointMode}
              baseUrl={baseUrl}
              generationUrl={generationUrl}
              editUrl={editUrl}
              urlSaved={urlSaved}
              onSelectImageModel={handleSelectImageModel}
              onSaveModel={() => void handleSaveModel()}
              onApiKeyChange={(nextKey) => {
                setApiKey(nextKey);
                setKeySaved(false);
              }}
              onShowKeyChange={setShowKey}
              onSaveKey={() => void handleSaveKey()}
              onEndpointModeChange={(mode) => {
                setEndpointMode(mode);
                setUrlSaved(false);
              }}
              onBaseUrlChange={(url) => {
                setBaseUrl(url);
                setUrlSaved(false);
              }}
              onGenerationUrlChange={(url) => {
                setGenerationUrl(url);
                setUrlSaved(false);
              }}
              onEditUrlChange={(url) => {
                setEditUrl(url);
                setUrlSaved(false);
              }}
              onSaveUrl={() => void handleSaveUrl()}
            />
          ) : (
            <LogsPanel
              t={t}
              logs={logs}
              totalLogs={totalLogs}
              logPage={logPage}
              totalPages={totalPages}
              logType={logType}
              logLevel={logLevel}
              logSettings={logSettings}
              selectedLog={selectedLog}
              responseContent={responseContent}
              runtimeLogs={runtimeLogs}
              runtimeLogsRef={runtimeLogsRef}
              autoScrollRuntimeLogs={autoScrollRuntimeLogs}
              copiedLogTarget={copiedLogTarget}
              onAutoScrollRuntimeLogsChange={setAutoScrollRuntimeLogs}
              onCopyText={(text, target) => void handleCopyText(text, target)}
              onClearRuntimeLogs={() => setRuntimeLogs([])}
              onLogTypeChange={(nextType) => {
                setLogType(nextType);
                setLogPage(1);
              }}
              onLogLevelChange={(nextLevel) => {
                setLogLevel(nextLevel);
                setLogPage(1);
              }}
              onSaveRetention={(days) => void handleSaveRetention(days)}
              onOpenClearLogs={() => setClearLogsOpen(true)}
              onSelectLog={(log) => void handleSelectLog(log)}
              onLogPageChange={setLogPage}
              onCloseSelectedLog={() => setSelectedLog(null)}
            />
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
