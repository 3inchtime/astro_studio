import { useEffect, useState, useCallback, useRef } from "react";
import { MotionConfig } from "framer-motion";
import { useNavigate } from "react-router-dom";
import { getVersion } from "@tauri-apps/api/app";
import {
  getLogs,
  clearLogs,
  getLogSettings,
  saveLogSettings,
  readLogResponseFile,
  getTrashSettings,
  saveTrashSettings,
  getFontSize,
  getImageModel,
  saveFontSize,
  saveImageModel,
  getRuntimeLogs,
  onRuntimeLog,
  getModelProviderProfiles,
  saveModelProviderProfiles,
  createModelProviderProfile,
  deleteModelProviderProfile,
  setActiveModelProvider,
} from "../lib/api";
import { Cpu, FileText, SlidersHorizontal } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useTheme } from "../hooks/useTheme";
import { useLayoutContext } from "../components/layout/AppLayout";
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
import { applyAppFontSize, getStoredAppFontSize } from "../lib/fontSize";
import { normalizeLanguage, type SupportedLanguage } from "../lib/languages";
import ConfirmDialog from "../components/common/ConfirmDialog";
import { GeneralSettingsPanel } from "../components/settings/GeneralSettingsPanel";
import { LogsPanel } from "../components/settings/LogsPanel";
import { ModelSettingsPanel } from "../components/settings/ModelSettingsPanel";
import { ROUTES } from "../app/routes/contracts";
import { copyTextToClipboard, mergeRuntimeLogs } from "../lib/settingsLogs";
import {
  DEFAULT_PROVIDER_ID,
  NEW_PROVIDER_NAME,
  defaultProviderProfilesStateForModel,
  providerForState,
  updateProviderInState,
} from "../lib/modelProviderProfiles";

const DEFAULT_MODEL: ImageModel = "gpt-image-2";
const FONT_SIZE_LABEL_KEYS: Record<AppFontSize, string> = {
  small: "settings.fontSizeSmall",
  medium: "settings.fontSizeMedium",
  large: "settings.fontSizeLarge",
};

const SETTINGS_TABS = [
  {
    id: "general",
    icon: SlidersHorizontal,
    labelKey: "settings.general",
    descriptionKey: "settings.generalDesc",
  },
  {
    id: "model",
    icon: Cpu,
    labelKey: "settings.modelConfig",
    descriptionKey: "settings.modelConfigDesc",
  },
  {
    id: "logs",
    icon: FileText,
    labelKey: "log.title",
    descriptionKey: "log.liveDesc",
  },
] as const;

export default function SettingsPage() {
  const navigate = useNavigate();
  const { checkForUpdates, updateSupported } = useLayoutContext();
  const [savingSettings, setSavingSettings] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [saveSucceeded, setSaveSucceeded] = useState(false);
  const [savedProviderState, setSavedProviderState] = useState(() =>
    defaultProviderProfilesStateForModel(DEFAULT_MODEL),
  );
  const [savedImageModel, setSavedImageModel] = useState<ImageModel>(DEFAULT_MODEL);
  const [savedTrashDays, setSavedTrashDays] = useState(30);
  function clearSaveFeedback() {
    setSaveError(false);
    setSaveSucceeded(false);
    retrySettingsRef.current = null;
  }
  const retrySettingsRef = useRef<(() => Promise<void>) | null>(null);
  const settingsPendingRef = useRef(false);
  async function runSettingsAction(action: () => Promise<void>) {
    if (settingsPendingRef.current) return;
    settingsPendingRef.current = true;
    setSavingSettings(true);
    setSaveError(false);
    setSaveSucceeded(false);
    retrySettingsRef.current = action;
    try { await action(); retrySettingsRef.current = null; setSaveSucceeded(true); }
    catch { setSaveError(true); }
    finally { settingsPendingRef.current = false; setSavingSettings(false); }
  }
  const [activeTab, setActiveTab] = useState<"general" | "model" | "logs">(
    "general",
  );
  const { theme, setThemeWithEvent } = useTheme();

  // General settings state
  const [showKey, setShowKey] = useState(false);
  const [providerState, setProviderState] =
    useState<ModelProviderProfilesState>(() =>
      defaultProviderProfilesStateForModel(DEFAULT_MODEL),
    );
  const [selectedProviderId, setSelectedProviderId] =
    useState(DEFAULT_PROVIDER_ID);
  const [providerSaved, setProviderSaved] = useState(false);
  const [imageModel, setImageModel] = useState<ImageModel>(DEFAULT_MODEL);
  const [modelSaved, setModelSaved] = useState(false);
  const { t, i18n } = useTranslation();
  const [language, setLanguage] = useState<SupportedLanguage>(() =>
    normalizeLanguage(i18n.resolvedLanguage ?? i18n.language),
  );
  const [fontSize, setFontSize] = useState<AppFontSize>(getStoredAppFontSize());
  const [fontSizeSaved, setFontSizeSaved] = useState(false);
  const [trashSettings, setTrashSettings] = useState<TrashSettings>({
    retention_days: 30,
  });
  const [trashSaved, setTrashSaved] = useState(false);
  const [updateChecking, setUpdateChecking] = useState(false);
  const [appVersion, setAppVersion] = useState("");

  // Logs state
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [totalLogs, setTotalLogs] = useState(0);
  const [logPage, setLogPage] = useState(1);
  const [logType, setLogType] = useState("");
  const [logLevel, setLogLevel] = useState("");
  const [logSettings, setLogSettings] = useState<LogSettings>({
    enabled: true,
    retention_days: 7,
  });
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null);
  const [responseContent, setResponseContent] = useState<string | null>(null);
  const [runtimeLogs, setRuntimeLogs] = useState<RuntimeLogEntry[]>([]);
  const [autoScrollRuntimeLogs, setAutoScrollRuntimeLogs] = useState(true);
  const [copiedLogTarget, setCopiedLogTarget] = useState<
    "runtime" | "detail" | null
  >(null);
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
    getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion(""));
  }, []);

  useEffect(() => {
    let cancelled = false;

    getImageModel()
      .then((model) => {
        if (cancelled || didUserSelectModelRef.current) {
          return;
        }

        setImageModel(model);
        setSavedImageModel(model);
      })
      .catch(() => {
        // Ignore persisted model load failures and keep catalog default.
      });

    getFontSize()
      .then((size) => {
        setFontSize(size);
        applyAppFontSize(size);
      })
      .catch(() => {
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
    setSavedProviderState(defaultState);
    setSelectedProviderId(defaultState.active_provider_id);

    getModelProviderProfiles(modelAtLoadStart)
      .then((state) => {
        if (cancelled || imageModelRef.current !== modelAtLoadStart) {
          return;
        }

        setProviderState(state);
        setSavedProviderState(state);
        setSelectedProviderId(state.active_provider_id);
      })
      .catch(() => {
        if (cancelled || imageModelRef.current !== modelAtLoadStart) {
          return;
        }

        setProviderState(defaultState);
        setSavedProviderState(defaultState);
        setSelectedProviderId(defaultState.active_provider_id);
      });

    return () => {
      cancelled = true;
    };
  }, [imageModel]);

  useEffect(() => {
    getLogSettings().then(setLogSettings).catch(() => {});
    getTrashSettings().then((settings) => { setTrashSettings(settings); setSavedTrashDays(settings.retention_days); }).catch(() => {});
  }, []);

  useEffect(() => {
    setLanguage(normalizeLanguage(i18n.resolvedLanguage ?? i18n.language));
  }, [i18n.language, i18n.resolvedLanguage]);

  const fetchLogs = useCallback(async () => {
    try {
      const result = await getLogs(
        logType || undefined,
        logLevel || undefined,
        logPage,
        pageSize,
      );
      setLogs(result.logs);
      setTotalLogs(result.total);
    } catch {
      /* ignore */
    }
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
    })
      .then((dispose) => {
        if (cancelled) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch(() => {
        unlisten = undefined;
      });

    getRuntimeLogs(200)
      .then((entries) => {
        if (cancelled) return;
        setRuntimeLogs((current) => mergeRuntimeLogs(current, entries));
      })
      .catch(() => {
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
    clearSaveFeedback();
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
    setSavedProviderState(nextState);
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
    const stateAtCreateStart = providerState;
    const nextState = await createModelProviderProfile(
      modelAtCreateStart,
      NEW_PROVIDER_NAME,
    );
    if (imageModelRef.current !== modelAtCreateStart) {
      return;
    }

    setProviderState(nextState);
    setSavedProviderState(nextState);
    const existingProviderIds = new Set(
      stateAtCreateStart.profiles.map((profile) => profile.id),
    );
    const createdProvider = nextState.profiles
      .filter((profile) => !existingProviderIds.has(profile.id))
      .slice(-1)[0];
    setSelectedProviderId(
      createdProvider?.id ??
        (providerForState(nextState, selectedProviderId)
          ? selectedProviderId
          : nextState.active_provider_id),
    );
    setShowKey(false);
    setProviderSaved(false);
  }

  async function handleDeleteProvider(providerId: string) {
    const modelAtDeleteStart = imageModel;
    const nextState = await deleteModelProviderProfile(
      modelAtDeleteStart,
      providerId,
    );
    if (imageModelRef.current !== modelAtDeleteStart) {
      return;
    }

    setProviderState(nextState);
    setSavedProviderState(nextState);
    setSelectedProviderId(
      providerForState(nextState, selectedProviderId)
        ? selectedProviderId
        : nextState.active_provider_id,
    );
  }

  async function handleSetActiveProvider(providerId: string) {
    const modelAtActivateStart = imageModel;
    const nextState = await setActiveModelProvider(
      modelAtActivateStart,
      providerId,
    );
    if (imageModelRef.current !== modelAtActivateStart) {
      return;
    }

    setProviderState(nextState);
    setSavedProviderState(nextState);
    setSelectedProviderId(providerId);
    setProviderSaved(false);
  }

  async function handleSaveModel() {
    await saveImageModel(imageModel);
    setSavedImageModel(imageModel);
    setModelSaved(true);
    setTimeout(() => setModelSaved(false), 2000);
  }

  async function handleCancelProviderEdit() {
    const modelAtCancelStart = imageModel;
    try {
      const state = await getModelProviderProfiles(modelAtCancelStart);
      if (imageModelRef.current !== modelAtCancelStart) return;
      setProviderState(state);
      setSavedProviderState(state);
      clearSaveFeedback();
      setSelectedProviderId(state.active_provider_id);
    } catch {
      // keep current state on failure
    }
    setShowKey(false);
  }

  function handleSelectImageModel(model: ImageModel) {
    clearSaveFeedback();
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
    setSavedTrashDays(trashSettings.retention_days);
    setTrashSaved(true);
    setTimeout(() => setTrashSaved(false), 2000);
  }

  async function handleCheckUpdate() {
    if (updateSupported !== true) {
      return;
    }

    setUpdateChecking(true);
    try {
      const update = await checkForUpdates();
      if (!update) {
        alert(t("update.noUpdate"));
      }
    } catch (err) {
      alert(t("update.checkFailed"));
    } finally {
      setUpdateChecking(false);
    }
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
      } catch {
        /* ignore */
      }
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
  const hasUnsavedChanges = activeTab === "model"
    ? imageModel !== savedImageModel || JSON.stringify(providerState) !== JSON.stringify(savedProviderState)
    : activeTab === "general" && trashSettings.retention_days !== savedTrashDays;

  return (
    <MotionConfig reducedMotion="user">
    <div className="h-full min-w-0 overflow-y-auto bg-background">
      <div className="mx-auto w-full max-w-[1120px] px-6 py-6">
        <header className="mb-6">
          <h1 className="text-xl font-semibold tracking-tight text-foreground">{t("settings.title")}</h1>
        </header>
        <div className="grid min-w-0 gap-8 md:grid-cols-[160px_minmax(0,1fr)]">
        <nav aria-label={t("settings.sections", { defaultValue: "Settings sections" })} className="flex flex-wrap content-start gap-1 md:flex-col">
          {SETTINGS_TABS.map(({ id, icon: Icon, labelKey }) => (
            <button key={id} type="button" aria-label={t(labelKey)} aria-current={activeTab === id ? "page" : undefined} onClick={() => setActiveTab(id)} className={`focus-ring flex min-h-9 items-center gap-2.5 rounded-lg border px-3 py-2 text-left text-sm font-medium transition-colors ${activeTab === id ? "border-border bg-surface text-foreground" : "border-transparent text-muted hover:bg-subtle hover:text-foreground"}`}>
              <Icon size={16} strokeWidth={1.8} /><span>{t(labelKey)}</span>
            </button>
          ))}
        </nav>
        <div className="min-w-0">
          {!savingSettings && !saveError && hasUnsavedChanges ? <p className="mb-4 text-sm text-muted">{t("studio.settings.unsaved", { defaultValue: "Unsaved changes" })}</p> : null}
          {!savingSettings && !saveError && !hasUnsavedChanges && saveSucceeded ? <p role="status" className="mb-4 text-sm text-success">{t("studio.settings.savedLocally", { defaultValue: "Changes saved." })}</p> : null}
          {savingSettings ? <p role="status" className="mb-4 rounded-lg border border-border-subtle bg-surface px-3 py-2 text-sm text-muted">{t("studio.settings.saving", { defaultValue: "Saving changes…" })}</p> : null}
          {saveError ? <div role="alert" className="mb-4 flex items-center justify-between gap-3 rounded-lg border border-error/25 bg-surface px-3 py-2 text-sm text-error">
            <span>{t("studio.settings.saveFailed", { defaultValue: "Could not save changes. Please try again." })}</span>
            <button type="button" onClick={() => { if (retrySettingsRef.current) void runSettingsAction(retrySettingsRef.current); }} className="focus-ring h-9 shrink-0 rounded-lg px-3 font-medium hover:bg-subtle">{t("studio.settings.retry", { defaultValue: "Retry" })}</button>
          </div> : null}
          <fieldset disabled={savingSettings} aria-busy={savingSettings} className="min-w-0 border-0 p-0">
          {activeTab === "general" ? (
            <GeneralSettingsPanel
              t={t}
              language={language}
              trashSettings={trashSettings}
              trashSaved={trashSaved}
              fontSize={fontSize}
              fontSizeSaved={fontSizeSaved}
              theme={theme}
              appVersion={appVersion}
              updateSupported={updateSupported === true}
              fontSizeLabelKeys={FONT_SIZE_LABEL_KEYS}
              onLanguageChange={handleLanguageChange}
              onTrashSettingsChange={(settings) => { clearSaveFeedback(); setTrashSaved(false); setTrashSettings(settings); }}
              onSaveTrashRetention={() => void runSettingsAction(() => handleSaveTrashRetention())}
              onOpenTrash={() => navigate(ROUTES.trash)}
              onFontSizeChange={(nextSize) =>
                void runSettingsAction(() => handleFontSizeChange(nextSize))
              }
              onThemeChange={(nextTheme, event) =>
                setThemeWithEvent(nextTheme, event)
              }
              onCheckUpdate={() => void handleCheckUpdate()}
              updateChecking={updateChecking}
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
              onSaveModel={() => void runSettingsAction(() => handleSaveModel())}
              onSelectProvider={(providerId) => {
                setSelectedProviderId(providerId);
                setShowKey(false);
                setProviderSaved(false);
              }}
              onProviderNameChange={(name) =>
                updateSelectedProvider((provider) => ({ ...provider, name }))
              }
              onProviderApiKeyChange={(apiKey) =>
                updateSelectedProvider((provider) => ({
                  ...provider,
                  api_key: apiKey,
                }))
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
              onCreateProvider={() => void runSettingsAction(() => handleCreateProvider())}
              onDeleteProvider={(providerId) =>
                void runSettingsAction(() => handleDeleteProvider(providerId))
              }
              onSetActiveProvider={(providerId) =>
                void runSettingsAction(() => handleSetActiveProvider(providerId))
              }
              onSaveProvider={() => void runSettingsAction(() => handleSaveProvider())}
              onCancelProviderEdit={() => void handleCancelProviderEdit()}
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
              onSaveRetention={(days) => void runSettingsAction(() => handleSaveRetention(days))}
              onOpenClearLogs={() => setClearLogsOpen(true)}
              onSelectLog={(log) => void handleSelectLog(log)}
              onLogPageChange={setLogPage}
              onCloseSelectedLog={() => setSelectedLog(null)}
            />
          )}
        </fieldset>
        </div>
        </div>
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
    </MotionConfig>
  );
}
