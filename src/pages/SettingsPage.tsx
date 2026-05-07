import { useEffect, useState, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useNavigate } from "react-router-dom";
import {
  getLogs, clearLogs, getLogSettings, saveLogSettings,
  readLogResponseFile, getTrashSettings, saveTrashSettings,
  getFontSize, getImageModel, saveFontSize, saveImageModel,
  getRuntimeLogs, onRuntimeLog, getModelProviderProfiles,
  saveModelProviderProfiles, createModelProviderProfile,
  deleteModelProviderProfile, setActiveModelProvider,
} from "../lib/api";
import {
  Cpu, FileText, SlidersHorizontal,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  AppFontSize,
  ImageModel,
  LogEntry,
  LogSettings,
  ModelProviderProfile,
  ModelProviderProfilesState,
  RuntimeLogEntry,
  TrashSettings,
} from "../types";
import {
  applyAppFontSize,
  getStoredAppFontSize,
} from "../lib/fontSize";
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
  DEFAULT_PROVIDER_ID,
  NEW_PROVIDER_NAME,
  defaultProviderProfilesStateForModel,
  providerForState,
  removeProviderFromState,
  updateProviderInState,
} from "../lib/modelProviderProfiles";

const DEFAULT_MODEL: ImageModel = "gpt-image-2";
const FONT_SIZE_LABEL_KEYS: Record<AppFontSize, string> = {
  small: "settings.fontSizeSmall",
  medium: "settings.fontSizeMedium",
  large: "settings.fontSizeLarge",
};

const SETTINGS_TABS = [
  { id: "general", icon: SlidersHorizontal, labelKey: "settings.general" },
  { id: "model", icon: Cpu, labelKey: "settings.modelConfig" },
  { id: "logs", icon: FileText, labelKey: "log.title" },
] as const;

export default function SettingsPage() {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<"general" | "model" | "logs">("general");

  // General settings state
  const [showKey, setShowKey] = useState(false);
  const [providerState, setProviderState] = useState<ModelProviderProfilesState>(() =>
    defaultProviderProfilesStateForModel(DEFAULT_MODEL),
  );
  const [selectedProviderId, setSelectedProviderId] = useState(DEFAULT_PROVIDER_ID);
  const [providerSaved, setProviderSaved] = useState(false);
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
    const modelAtLoadStart = imageModel;
    const defaultState = defaultProviderProfilesStateForModel(modelAtLoadStart);

    setShowKey(false);
    setProviderSaved(false);
    setProviderState(defaultState);
    setSelectedProviderId(defaultState.active_provider_id);

    getModelProviderProfiles(modelAtLoadStart).then((state) => {
      if (cancelled || imageModelRef.current !== modelAtLoadStart) {
        return;
      }

      setProviderState(state);
      setSelectedProviderId(state.active_provider_id);
    }).catch(() => {
      if (cancelled || imageModelRef.current !== modelAtLoadStart) {
        return;
      }

      setProviderState(defaultState);
      setSelectedProviderId(defaultState.active_provider_id);
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

  function updateSelectedProvider(
    update: (profile: ModelProviderProfile) => ModelProviderProfile,
  ) {
    setProviderState((currentState) => {
      if (!providerForState(currentState, selectedProviderId)) {
        return currentState;
      }

      return updateProviderInState(currentState, selectedProviderId, update);
    });
    setProviderSaved(false);
  }

  async function handleSaveProvider() {
    const modelAtSaveStart = imageModel;
    const stateAtSaveStart = providerState;
    const nextState = await saveModelProviderProfiles(
      modelAtSaveStart,
      stateAtSaveStart,
    );
    if (imageModelRef.current !== modelAtSaveStart) {
      return;
    }

    setProviderState(nextState);
    setSelectedProviderId(
      providerForState(nextState, selectedProviderId)
        ? selectedProviderId
        : nextState.active_provider_id,
    );
    setShowKey(false);
    setProviderSaved(true);
    setTimeout(() => setProviderSaved(false), 2000);
  }

  async function handleCreateProvider() {
    const modelAtCreateStart = imageModel;
    const nextState = await createModelProviderProfile(
      modelAtCreateStart,
      NEW_PROVIDER_NAME,
    );
    if (imageModelRef.current !== modelAtCreateStart) {
      return;
    }

    setProviderState(nextState);
    setSelectedProviderId(nextState.active_provider_id);
    setShowKey(false);
    setProviderSaved(false);
  }

  async function handleDeleteProvider(providerId: string) {
    const modelAtDeleteStart = imageModel;
    setProviderState((currentState) => {
      const nextState = removeProviderFromState(currentState, providerId);
      if (!providerForState(nextState, selectedProviderId)) {
        setSelectedProviderId(nextState.active_provider_id);
      }

      return nextState;
    });
    setProviderSaved(false);

    const nextState = await deleteModelProviderProfile(modelAtDeleteStart, providerId);
    if (imageModelRef.current !== modelAtDeleteStart) {
      return;
    }

    setProviderState(nextState);
    setSelectedProviderId(
      providerForState(nextState, selectedProviderId)
        ? selectedProviderId
        : nextState.active_provider_id,
    );
  }

  async function handleSetActiveProvider(providerId: string) {
    const modelAtActivateStart = imageModel;
    const nextState = await setActiveModelProvider(modelAtActivateStart, providerId);
    if (imageModelRef.current !== modelAtActivateStart) {
      return;
    }

    setProviderState(nextState);
    setSelectedProviderId(providerId);
    setProviderSaved(false);
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
    setProviderSaved(false);
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
              showKey={showKey}
              providerState={providerState}
              selectedProviderId={selectedProviderId}
              providerSaved={providerSaved}
              onSelectImageModel={handleSelectImageModel}
              onSaveModel={() => void handleSaveModel()}
              onSelectProvider={(providerId) => {
                setSelectedProviderId(providerId);
                setShowKey(false);
                setProviderSaved(false);
              }}
              onProviderNameChange={(name) =>
                updateSelectedProvider((provider) => ({ ...provider, name }))
              }
              onProviderApiKeyChange={(apiKey) =>
                updateSelectedProvider((provider) => ({ ...provider, api_key: apiKey }))
              }
              onShowKeyChange={setShowKey}
              onProviderEndpointModeChange={(mode) =>
                updateSelectedProvider((provider) => ({
                  ...provider,
                  endpoint_settings: {
                    ...provider.endpoint_settings,
                    mode,
                  },
                }))
              }
              onProviderBaseUrlChange={(url) =>
                updateSelectedProvider((provider) => ({
                  ...provider,
                  endpoint_settings: {
                    ...provider.endpoint_settings,
                    base_url: url,
                  },
                }))
              }
              onProviderGenerationUrlChange={(url) =>
                updateSelectedProvider((provider) => ({
                  ...provider,
                  endpoint_settings: {
                    ...provider.endpoint_settings,
                    generation_url: url,
                  },
                }))
              }
              onProviderEditUrlChange={(url) =>
                updateSelectedProvider((provider) => ({
                  ...provider,
                  endpoint_settings: {
                    ...provider.endpoint_settings,
                    edit_url: url,
                  },
                }))
              }
              onCreateProvider={() => void handleCreateProvider()}
              onDeleteProvider={(providerId) => void handleDeleteProvider(providerId)}
              onSetActiveProvider={(providerId) => void handleSetActiveProvider(providerId)}
              onSaveProvider={() => void handleSaveProvider()}
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
