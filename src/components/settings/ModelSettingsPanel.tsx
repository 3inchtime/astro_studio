import { motion } from "framer-motion";
import { Check, Cpu, Eye, EyeOff, Globe, Plus, Trash2 } from "lucide-react";
import type { TFunction } from "i18next";
import type {
  EndpointMode,
  ImageModel,
  ModelProviderProfilesState,
} from "../../types";
import {
  IMAGE_MODEL_CATALOG,
  getImageModelCatalogEntry,
} from "../../lib/modelCatalog";
import {
  defaultBaseUrlForModel,
  defaultEditUrlForModel,
  defaultGenerationUrlForModel,
  modelSupportsEdit,
  usesSharedEditEndpoint,
} from "../../lib/settingsEndpoints";
import { cardVariants, sectionVariants } from "./settingsMotion";

interface ModelSettingsPanelProps {
  t: TFunction;
  imageModel: ImageModel;
  modelSaved: boolean;
  showKey: boolean;
  providerState: ModelProviderProfilesState;
  selectedProviderId: string;
  providerSaved: boolean;
  onSelectImageModel: (model: ImageModel) => void;
  onSaveModel: () => void;
  onSelectProvider: (providerId: string) => void;
  onProviderNameChange: (name: string) => void;
  onProviderApiKeyChange: (apiKey: string) => void;
  onShowKeyChange: (showKey: boolean) => void;
  onProviderEndpointModeChange: (mode: EndpointMode) => void;
  onProviderBaseUrlChange: (url: string) => void;
  onProviderGenerationUrlChange: (url: string) => void;
  onProviderEditUrlChange: (url: string) => void;
  onCreateProvider: () => void;
  onDeleteProvider: (providerId: string) => void;
  onSetActiveProvider: (providerId: string) => void;
  onSaveProvider: () => void;
}

function formatProviderName(provider: string): string {
  return provider.charAt(0).toUpperCase() + provider.slice(1);
}

function maskKey(key: string): string {
  if (key.length <= 8) return "sk-****";
  return key.slice(0, 3) + "..." + key.slice(-4);
}

export function ModelSettingsPanel({
  t,
  imageModel,
  modelSaved,
  showKey,
  providerState,
  selectedProviderId,
  providerSaved,
  onSelectImageModel,
  onSaveModel,
  onSelectProvider,
  onProviderNameChange,
  onProviderApiKeyChange,
  onShowKeyChange,
  onProviderEndpointModeChange,
  onProviderBaseUrlChange,
  onProviderGenerationUrlChange,
  onProviderEditUrlChange,
  onCreateProvider,
  onDeleteProvider,
  onSetActiveProvider,
  onSaveProvider,
}: ModelSettingsPanelProps) {
  const selectedProvider =
    providerState.profiles.find((provider) => provider.id === selectedProviderId) ??
    providerState.profiles[0];
  const endpointSettings = selectedProvider.endpoint_settings;
  const apiKey = selectedProvider.api_key;
  const displayKey = showKey ? apiKey : (apiKey ? maskKey(apiKey) : "");
  const canDeleteProvider = providerState.profiles.length > 1;

  return (
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
                      onClick={() => onSelectImageModel(entry.id)}
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
                  onClick={onSaveModel}
                  whileTap={{ scale: 0.97 }}
                  className="flex h-[32px] shrink-0 items-center justify-center gap-1.5 rounded-[8px] border border-border-subtle bg-surface px-3 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
                >
                  {modelSaved ? (<><Check size={13} className="text-success" /><span className="text-success">{t("settings.saved")}</span></>) : t("settings.saveModel")}
                </motion.button>
              </div>
            </div>
          </div>
          <div className="border-t border-border-subtle" />
          <div className="grid gap-4 p-5 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-start lg:gap-6">
            <div className="flex items-start gap-3">
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-primary/10 bg-primary/5">
                <Globe size={14} className="text-primary" strokeWidth={2} />
              </div>
              <div>
                <h4 className="text-[13px] font-semibold text-foreground">{t("settings.providers")}</h4>
                <p className="mt-0.5 text-[11px] leading-relaxed text-muted/60">{t("settings.providersDesc")}</p>
              </div>
            </div>
            <div className="grid min-w-0 gap-4 xl:grid-cols-[240px_minmax(0,1fr)]">
              <div className="min-w-0 space-y-2">
                <button
                  type="button"
                  onClick={onCreateProvider}
                  aria-label={t("settings.newProvider")}
                  className="flex h-[36px] w-full items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
                >
                  <Plus size={13} />
                  {t("settings.newProvider")}
                </button>
                <div className="space-y-2">
                  {providerState.profiles.map((provider) => {
                    const selected = provider.id === selectedProvider.id;
                    const active = provider.id === providerState.active_provider_id;

                    return (
                      <div
                        key={provider.id}
                        role="button"
                        tabIndex={0}
                        aria-label={`Select ${provider.name} provider`}
                        aria-pressed={selected}
                        onClick={() => onSelectProvider(provider.id)}
                        onKeyDown={(event) => {
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault();
                            onSelectProvider(provider.id);
                          }
                        }}
                        className={`rounded-[10px] border p-3 text-left transition-all ${
                          selected
                            ? "border-primary/35 bg-primary/6 shadow-card"
                            : "border-border-subtle bg-subtle/20 hover:border-border hover:bg-subtle/35"
                        }`}
                      >
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0">
                            <p className="truncate text-[12px] font-semibold text-foreground">{provider.name}</p>
                            <p className="mt-1 truncate font-mono text-[10.5px] text-muted/55">
                              {provider.api_key ? maskKey(provider.api_key) : t("settings.noApiKey")}
                            </p>
                          </div>
                          {active && (
                            <span className="shrink-0 rounded-[6px] border border-primary/15 bg-primary/8 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                              {t("settings.activeProvider")}
                            </span>
                          )}
                        </div>
                        <div className="mt-3 flex items-center justify-end gap-2">
                          {!active && (
                            <button
                              type="button"
                              aria-label={`Use ${provider.name} provider`}
                              onClick={(event) => {
                                event.stopPropagation();
                                onSetActiveProvider(provider.id);
                              }}
                              className="flex h-[28px] items-center justify-center gap-1 rounded-[7px] border border-border-subtle bg-surface px-2 text-[11px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
                            >
                              <Check size={12} />
                              {t("settings.useProvider")}
                            </button>
                          )}
                          <button
                            type="button"
                            aria-label={`Delete ${provider.name} provider`}
                            disabled={!canDeleteProvider}
                            onClick={(event) => {
                              event.stopPropagation();
                              onDeleteProvider(provider.id);
                            }}
                            className="flex h-[28px] w-[28px] items-center justify-center rounded-[7px] border border-border-subtle bg-surface text-muted/55 transition-all hover:border-border hover:text-foreground disabled:opacity-30"
                          >
                            <Trash2 size={12} />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              <div className="min-w-0 space-y-3 rounded-[10px] border border-border-subtle bg-subtle/20 p-3">
                <div className="grid gap-2 sm:grid-cols-2">
                  <label className="grid gap-1.5">
                    <span className="text-[11px] font-medium text-muted/70">{t("settings.providerName")}</span>
                    <input
                      type="text"
                      value={selectedProvider.name}
                      onChange={(e) => onProviderNameChange(e.target.value)}
                      placeholder={t("settings.providerNamePlaceholder")}
                      className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                    />
                  </label>
                  <label className="grid gap-1.5">
                    <span className="text-[11px] font-medium text-muted/70">{t("settings.apiKey")}</span>
                    <div className="relative min-w-0">
                      <input
                        type={showKey ? "text" : "password"}
                        value={displayKey}
                        onChange={(e) => onProviderApiKeyChange(e.target.value)}
                        onFocus={() => { if (!showKey) onShowKeyChange(true); }}
                        placeholder={t("settings.apiKeyPlaceholder")}
                        className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 pr-9 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => onShowKeyChange(!showKey)}
                        title={showKey ? t("settings.hideKey") : t("settings.showKey")}
                        aria-label={showKey ? t("settings.hideKey") : t("settings.showKey")}
                        className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-muted/40 transition-colors hover:bg-subtle hover:text-muted"
                      >
                        {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
                      </button>
                    </div>
                  </label>
                </div>

                <div className="grid gap-2 rounded-[10px] border border-border-subtle bg-surface p-1 sm:grid-cols-2">
                  {(["base_url", "full_url"] as EndpointMode[]).map((mode) => (
                    <button
                      key={mode}
                      type="button"
                      onClick={() => onProviderEndpointModeChange(mode)}
                      className={`h-[34px] rounded-[8px] px-3 text-[12px] font-medium transition-all ${
                        endpointSettings.mode === mode
                          ? "bg-subtle text-foreground shadow-card"
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

                {endpointSettings.mode === "base_url" ? (
                  <input
                    type="text"
                    value={endpointSettings.base_url}
                    onChange={(e) => onProviderBaseUrlChange(e.target.value)}
                    placeholder={defaultBaseUrlForModel(imageModel)}
                    className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                  />
                ) : (
                  <div className="grid gap-2">
                    <label className="grid gap-1.5">
                      <span className="text-[11px] font-medium text-muted/70">{t("settings.generationUrl")}</span>
                      <input
                        type="text"
                        value={endpointSettings.generation_url}
                        onChange={(e) => onProviderGenerationUrlChange(e.target.value)}
                        placeholder={defaultGenerationUrlForModel(imageModel)}
                        className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                      />
                    </label>
                    {modelSupportsEdit(imageModel) && !usesSharedEditEndpoint(imageModel) && (
                      <label className="grid gap-1.5">
                        <span className="text-[11px] font-medium text-muted/70">{t("settings.editUrl")}</span>
                        <input
                          type="text"
                          value={endpointSettings.edit_url}
                          onChange={(e) => onProviderEditUrlChange(e.target.value)}
                          placeholder={defaultEditUrlForModel(imageModel)}
                          className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
                        />
                      </label>
                    )}
                  </div>
                )}

                <div className="flex flex-wrap justify-end gap-2">
                  {selectedProvider.id !== providerState.active_provider_id && (
                    <motion.button
                      type="button"
                      onClick={() => onSetActiveProvider(selectedProvider.id)}
                      whileTap={{ scale: 0.97 }}
                      className="flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle bg-surface px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
                    >
                      <Check size={13} />
                      {t("settings.activateProvider")}
                    </motion.button>
                  )}
                  <motion.button
                    type="button"
                    onClick={onSaveProvider}
                    whileTap={{ scale: 0.97 }}
                    className="flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle bg-surface px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground lg:min-w-[104px]"
                  >
                    {providerSaved ? (<><Check size={13} className="text-success" /><span className="text-success">{t("settings.saved")}</span></>) : t("settings.saveProvider")}
                  </motion.button>
                </div>
              </div>
            </div>
          </div>
        </motion.div>
      </motion.section>
    </motion.div>
  );
}
