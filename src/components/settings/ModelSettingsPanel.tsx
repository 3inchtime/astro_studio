import { motion } from "framer-motion";
import { Check, Eye, EyeOff, Plus, Trash2, X } from "lucide-react";
import type { TFunction } from "i18next";
import type {
  EndpointMode,
  ImageModel,
  ModelProviderProfilesState,
} from "../../types";
import { IMAGE_MODEL_CATALOG } from "../../lib/modelCatalog";
import {
  defaultBaseUrlForModel,
  defaultEditUrlForModel,
  defaultGenerationUrlForModel,
  modelSupportsEdit,
  usesSharedEditEndpoint,
} from "../../lib/settingsEndpoints";
import { cardVariants, sectionVariants } from "./settingsMotion";
import { LlmConfigSection } from "./LlmConfigSection";

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
  onCancelProviderEdit: () => void;
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
  onCancelProviderEdit,
}: ModelSettingsPanelProps) {
  const selectedProvider =
    providerState.profiles.find((provider) => provider.id === selectedProviderId) ??
    providerState.profiles[0];
  if (!selectedProvider) {
    return (
      <motion.div
        key="model"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.18 }}
      >
        <motion.section
          custom={0}
          variants={sectionVariants}
          initial="hidden"
          animate="visible"
          className="space-y-4"
        >
          <motion.div
            custom={0}
            variants={cardVariants}
            initial="hidden"
            animate="visible"
            className="border-b border-border-subtle pb-6"
          >
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <h3 className="text-[20px] font-semibold leading-tight text-foreground">
                {t("settings.imageGenerationConfig")}
              </h3>
              <motion.button
                type="button"
                onClick={onSaveModel}
                    className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-primary/20 bg-primary/10 px-3 text-[14px] font-medium text-primary transition-colors hover:border-primary/35 hover:bg-primary/15"
              >
                {modelSaved ? (
                  <>
                    <Check size={13} className="text-success" />
                    <span className="text-success">{t("settings.saved")}</span>
                  </>
                ) : (
                  t("settings.saveModel")
                )}
              </motion.button>
            </div>

            <div className="grid gap-4">
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
                      className={`group flex min-h-[52px] items-center gap-3 rounded-[12px] border p-3 text-left transition-colors ${
                        active
                          ? "border-primary/35 bg-primary/6 "
                          : "border-border-subtle bg-subtle/15 hover:border-border hover:bg-subtle/35"
                      }`}
                    >
                      <span className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-[12px] font-semibold text-surface ${
                        entry.provider === "openai"
                          ? "bg-foreground"
                          : "bg-accent"
                      }`}>
                        {formatProviderName(entry.provider).charAt(0)}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-[14px] font-semibold text-foreground">{entry.label}</span>
                      </span>
                      <span className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors ${
                        active
                          ? "border-primary bg-primary text-white"
                          : "border-border-subtle text-transparent group-hover:border-border"
                      }`}>
                        <Check size={12} strokeWidth={3} />
                      </span>
                    </button>
                  );
                })}
              </div>

              <div className="grid gap-4 xl:grid-cols-[240px_minmax(0,1fr)] xl:items-stretch">
                <div className="space-y-3 border-border-subtle xl:border-r xl:pr-4">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <h4 className="text-[14px] font-semibold text-foreground">{t("settings.providers")}</h4>
                      <p className="mt-1 text-[12px] leading-relaxed text-muted">{t("settings.providersDesc")}</p>
                    </div>
                    <button
                      type="button"
                      onClick={onCreateProvider}
                      aria-label={t("settings.newProvider")}
                      className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-surface text-muted transition-colors hover:border-border hover:text-foreground"
                    >
                      <Plus size={14} />
                    </button>
                  </div>
                </div>

                <div className="flex min-h-[220px] flex-col items-center justify-center rounded-[12px] border border-dashed border-border-subtle bg-surface text-center">
                  <p className="text-[12px] text-muted">{t("settings.providersDesc")}</p>
                  <motion.button
                    type="button"
                    onClick={onCreateProvider}
                            className="mt-3 flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-primary/20 bg-primary/10 px-4 text-[14px] disabled:cursor-not-allowed disabled:opacity-50 font-medium text-primary transition-colors hover:border-primary/35 hover:bg-primary/15"
                  >
                    <Plus size={13} />
                    {t("settings.newProvider")}
                  </motion.button>
                </div>
              </div>
            </div>
          </motion.div>

          <LlmConfigSection />
        </motion.section>
      </motion.div>
    );
  }
  const endpointSettings = selectedProvider.endpoint_settings;
  // Empty URLs use the model defaults. Only validate fields used by this mode.
  const validEndpoint = (value: string) => {
    if (!value.trim()) return true;
    try { return ["https:", "http:"].includes(new URL(value.trim()).protocol); }
    catch { return false; }
  };
  const invalidName = !selectedProvider.name.trim();
  const invalidBaseUrl = endpointSettings.mode === "base_url" && !validEndpoint(endpointSettings.base_url);
  const invalidGenerationUrl = endpointSettings.mode === "full_url" && !validEndpoint(endpointSettings.generation_url);
  const invalidEditUrl = endpointSettings.mode === "full_url" && modelSupportsEdit(imageModel) && !usesSharedEditEndpoint(imageModel) && !validEndpoint(endpointSettings.edit_url);
  const invalidProvider = invalidName || invalidBaseUrl || invalidGenerationUrl || invalidEditUrl;
  const apiKey = selectedProvider.api_key;
  const displayKey = showKey ? apiKey : (apiKey ? maskKey(apiKey) : "");
  const selectedProviderIsActive = selectedProvider.id === providerState.active_provider_id;

  return (
    <motion.div
      key="model"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.18 }}
    >
      <motion.section
        custom={0}
        variants={sectionVariants}
        initial="hidden"
        animate="visible"
        className="space-y-4"
      >
        <motion.div
          custom={0}
          variants={cardVariants}
          initial="hidden"
          animate="visible"
          className="border-b border-border-subtle pb-6"
        >
          <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
            <h3 className="text-[20px] font-semibold leading-tight text-foreground">
              {t("settings.imageGenerationConfig")}
            </h3>
            <motion.button
              type="button"
              onClick={onSaveModel}
                className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-primary/20 bg-primary/10 px-3 text-[14px] font-medium text-primary transition-colors hover:border-primary/35 hover:bg-primary/15"
            >
              {modelSaved ? (
                <>
                  <Check size={13} className="text-success" />
                  <span className="text-success">{t("settings.saved")}</span>
                </>
              ) : (
                t("settings.saveModel")
              )}
            </motion.button>
          </div>

          <div className="grid gap-4">
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
                      className={`group flex min-h-[52px] items-center gap-3 rounded-[12px] border p-3 text-left transition-colors ${
                        active
                          ? "border-primary/35 bg-primary/6 "
                          : "border-border-subtle bg-subtle/15 hover:border-border hover:bg-subtle/35"
                      }`}
                    >
                      <span className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-[12px] font-semibold text-surface ${
                        entry.provider === "openai"
                          ? "bg-foreground"
                          : "bg-accent"
                      }`}>
                        {formatProviderName(entry.provider).charAt(0)}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-[14px] font-semibold text-foreground">{entry.label}</span>
                      </span>
                      <span className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors ${
                        active
                          ? "border-primary bg-primary text-white"
                          : "border-border-subtle text-transparent group-hover:border-border"
                      }`}>
                        <Check size={12} strokeWidth={3} />
                      </span>
                    </button>
                  );
                })}
              </div>

            <div className="grid gap-4 xl:grid-cols-[240px_minmax(0,1fr)] xl:items-stretch">
              <div className="space-y-3 border-border-subtle xl:border-r xl:pr-4">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <h4 className="text-[14px] font-semibold text-foreground">{t("settings.providers")}</h4>
                    <p className="mt-1 text-[12px] leading-relaxed text-muted">{t("settings.providersDesc")}</p>
                  </div>
                  <button
                    type="button"
                    onClick={onCreateProvider}
                    aria-label={t("settings.newProvider")}
                    className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-surface text-muted transition-colors hover:border-border hover:text-foreground"
                  >
                    <Plus size={14} />
                  </button>
                </div>
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
                        className={`rounded-lg border p-3 text-left transition-colors ${
                          selected
                            ? "border-primary/35 bg-primary/6 "
                            : "border-border-subtle bg-surface hover:border-border hover:bg-subtle/25"
                        }`}
                      >
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0">
                            <p className="truncate text-[12px] font-semibold text-foreground">{provider.name}</p>
                            <p className="mt-1 truncate font-mono text-[12px] text-muted">
                              {provider.api_key ? maskKey(provider.api_key) : t("settings.noApiKey")}
                            </p>
                          </div>
                          <div className="flex shrink-0 items-center gap-1.5">
                            {selected && (
                              <span className="flex h-5 w-5 items-center justify-center rounded-full border border-primary bg-primary text-white">
                                <Check size={12} strokeWidth={3} />
                              </span>
                            )}
                            {active && (
                              <span className="rounded-[6px] border border-primary/15 bg-primary/8 px-1.5 py-0.5 text-[12px] font-medium text-primary">
                                {t("settings.activeProvider")}
                              </span>
                            )}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

            <div className="min-w-0 space-y-4">
              <div>
                <h4 className="text-[14px] font-semibold text-foreground">{selectedProvider.name}</h4>
                <p className="mt-0.5 text-[12px] leading-relaxed text-muted">{t("settings.endpointDesc")}</p>
              </div>

              <div className="min-w-0 space-y-3">
                <div className="grid gap-3 border-b border-border-subtle pb-4">
                  <div className="grid gap-2 sm:grid-cols-2">
                  <label className="grid gap-1.5">
                    <span className="text-[12px] font-medium text-muted">{t("settings.providerName")}</span>
                    <input
                      type="text"
                      value={selectedProvider.name}
                      aria-invalid={invalidName}
                      aria-describedby={invalidName ? "provider-name-error" : undefined}
                      onChange={(e) => onProviderNameChange(e.target.value)}
                      placeholder={t("settings.providerNamePlaceholder")}
                      className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-[14px] placeholder:text-muted"
                    />
                    {invalidName && <span id="provider-name-error" className="text-xs text-error">{t("studio.settings.nameRequired", { defaultValue: "Enter a provider name." })}</span>}
                  </label>
                  <label className="grid gap-1.5">
                    <span className="text-[12px] font-medium text-muted">{t("settings.apiKey")}</span>
                    <div className="relative min-w-0">
                      <input
                        type={showKey ? "text" : "password"}
                        value={displayKey}
                        onChange={(e) => onProviderApiKeyChange(e.target.value)}
                        onFocus={() => { if (!showKey) onShowKeyChange(true); }}
                        placeholder={t("settings.apiKeyPlaceholder")}
                        className="studio-input focus-ring h-9 w-full rounded-lg px-3 pr-9 text-[14px] placeholder:text-muted"
                      />
                      <button
                        type="button"
                        onClick={() => onShowKeyChange(!showKey)}
                        title={showKey ? t("settings.hideKey") : t("settings.showKey")}
                        aria-label={showKey ? t("settings.hideKey") : t("settings.showKey")}
                        className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-muted transition-colors hover:bg-subtle hover:text-muted"
                      >
                        {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
                      </button>
                    </div>
                  </label>
                </div>
                </div>

                <div className="grid gap-3 border-b border-border-subtle pb-4">
                  <div>
                    <h5 className="text-[12px] font-semibold text-foreground">{t("settings.endpoint")}</h5>
                    <p className="mt-1 text-[12px] leading-relaxed text-muted">
                      {t("settings.endpointModeHint")}
                      {" "}{t("studio.settings.connectionHint", { defaultValue: "Saving stores these settings locally. It does not test the connection." })}
                    </p>
                  </div>

                  <div className="grid gap-2 rounded-lg border border-border-subtle bg-surface p-1 sm:grid-cols-2">
                    {(["base_url", "full_url"] as EndpointMode[]).map((mode) => (
                      <button
                        key={mode}
                        type="button"
                        onClick={() => onProviderEndpointModeChange(mode)}
                        className={`h-9 rounded-[8px] px-3 text-[12px] font-medium transition-colors ${
                          endpointSettings.mode === mode
                            ? "bg-subtle text-foreground "
                            : "text-muted hover:text-foreground"
                        }`}
                      >
                        {t(mode === "base_url" ? "settings.endpointBaseUrlMode" : "settings.endpointFullUrlMode")}
                      </button>
                    ))}
                  </div>

                  {endpointSettings.mode === "base_url" ? (
                    <label className="grid gap-1.5">
                      <span className="text-[12px] font-medium text-muted">{t("settings.endpoint")}</span>
                      <input
                        type="text"
                        value={endpointSettings.base_url}
                        aria-label={t("settings.endpoint")}
                        aria-invalid={invalidBaseUrl}
                        aria-describedby={invalidBaseUrl ? "provider-base-url-error" : undefined}
                        onChange={(e) => onProviderBaseUrlChange(e.target.value)}
                        placeholder={defaultBaseUrlForModel(imageModel)}
                        className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-[14px] placeholder:text-muted"
                      />
                      {invalidBaseUrl && <span id="provider-base-url-error" className="text-xs text-error">{t("studio.settings.invalidEndpoint", { defaultValue: "Enter an HTTP or HTTPS URL." })}</span>}
                    </label>
                  ) : (
                    <div className="grid gap-2">
                      <label className="grid gap-1.5">
                        <span className="text-[12px] font-medium text-muted">{t("settings.generationUrl")}</span>
                        <input
                          type="text"
                          value={endpointSettings.generation_url}
                        aria-invalid={invalidGenerationUrl}
                        aria-describedby={invalidGenerationUrl ? "provider-generation-url-error" : undefined}
                          onChange={(e) => onProviderGenerationUrlChange(e.target.value)}
                          placeholder={defaultGenerationUrlForModel(imageModel)}
                          className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-[14px] placeholder:text-muted"
                        />
                      {invalidGenerationUrl && <span id="provider-generation-url-error" className="text-xs text-error">{t("studio.settings.invalidEndpoint", { defaultValue: "Enter an HTTP or HTTPS URL." })}</span>}
                      </label>
                      {modelSupportsEdit(imageModel) && !usesSharedEditEndpoint(imageModel) && (
                        <label className="grid gap-1.5">
                          <span className="text-[12px] font-medium text-muted">{t("settings.editUrl")}</span>
                          <input
                            type="text"
                            value={endpointSettings.edit_url}
                        aria-invalid={invalidEditUrl}
                        aria-describedby={invalidEditUrl ? "provider-edit-url-error" : undefined}
                            onChange={(e) => onProviderEditUrlChange(e.target.value)}
                            placeholder={defaultEditUrlForModel(imageModel)}
                            className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-[14px] placeholder:text-muted"
                          />
                      {invalidEditUrl && <span id="provider-edit-url-error" className="text-xs text-error">{t("studio.settings.invalidEndpoint", { defaultValue: "Enter an HTTP or HTTPS URL." })}</span>}
                        </label>
                      )}
                    </div>
                  )}
                </div>

                <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border-subtle pt-3">
                  <p className="text-[12px] text-muted">
                    {selectedProviderIsActive
                      ? `${selectedProvider.name} · ${t("settings.activeProvider")}`
                      : `${selectedProvider.name} · ${t("settings.providers")}`}
                  </p>
                  <div className="flex flex-wrap justify-end gap-2">
                    {selectedProvider.id !== providerState.active_provider_id && (
                      <motion.button
                        type="button"
                        onClick={() => onSetActiveProvider(selectedProvider.id)}
                                    className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-primary/20 bg-primary/10 px-4 text-[14px] disabled:cursor-not-allowed disabled:opacity-50 font-medium text-primary transition-colors hover:border-primary/35 hover:bg-primary/15 lg:min-w-[104px]"
                      >
                        <Check size={13} />
                        {t("settings.activateProvider")}
                      </motion.button>
                    )}
                    <motion.button
                      type="button"
                      onClick={onSaveProvider}
                      disabled={invalidProvider}
                                className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-primary/20 bg-primary/10 px-4 text-[14px] disabled:cursor-not-allowed disabled:opacity-50 font-medium text-primary transition-colors hover:border-primary/35 hover:bg-primary/15 lg:min-w-[104px]"
                    >
                      {providerSaved ? (
                        <>
                          <Check size={13} className="text-success" />
                          <span className="text-success">{t("settings.saved")}</span>
                        </>
                      ) : (
                        t("settings.saveProvider")
                      )}
                    </motion.button>
                    <motion.button
                      type="button"
                      onClick={onCancelProviderEdit}
                                className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-border-subtle bg-surface px-4 text-[14px] font-medium text-muted transition-colors hover:border-border hover:text-foreground lg:min-w-[104px]"
                    >
                      <X size={13} />
                      {t("settings.cancelEdit")}
                    </motion.button>
                    <motion.button
                      type="button"
                      onClick={() => onDeleteProvider(selectedProvider.id)}
                                className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-error/20 bg-error/5 px-4 text-[14px] font-medium text-error transition-colors hover:border-error/30 hover:bg-error/10"
                    >
                      <Trash2 size={13} />
                      {t("settings.deleteProvider")}
                    </motion.button>
                  </div>
                </div>
              </div>
            </div>
          </div>
          </div>
        </motion.div>

        <LlmConfigSection />
      </motion.section>
    </motion.div>
  );
}
