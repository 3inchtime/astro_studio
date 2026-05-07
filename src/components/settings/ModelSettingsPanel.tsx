import { motion } from "framer-motion";
import { Check, Cpu, Eye, EyeOff, Globe, Key } from "lucide-react";
import type { TFunction } from "i18next";
import type { EndpointMode, ImageModel } from "../../types";
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
  apiKey: string;
  displayKey: string;
  showKey: boolean;
  keySaved: boolean;
  endpointMode: EndpointMode;
  baseUrl: string;
  generationUrl: string;
  editUrl: string;
  urlSaved: boolean;
  onSelectImageModel: (model: ImageModel) => void;
  onSaveModel: () => void;
  onApiKeyChange: (apiKey: string) => void;
  onShowKeyChange: (showKey: boolean) => void;
  onSaveKey: () => void;
  onEndpointModeChange: (mode: EndpointMode) => void;
  onBaseUrlChange: (url: string) => void;
  onGenerationUrlChange: (url: string) => void;
  onEditUrlChange: (url: string) => void;
  onSaveUrl: () => void;
}

function formatProviderName(provider: string): string {
  return provider.charAt(0).toUpperCase() + provider.slice(1);
}

export function ModelSettingsPanel({
  t,
  imageModel,
  modelSaved,
  apiKey,
  displayKey,
  showKey,
  keySaved,
  endpointMode,
  baseUrl,
  generationUrl,
  editUrl,
  urlSaved,
  onSelectImageModel,
  onSaveModel,
  onApiKeyChange,
  onShowKeyChange,
  onSaveKey,
  onEndpointModeChange,
  onBaseUrlChange,
  onGenerationUrlChange,
  onEditUrlChange,
  onSaveUrl,
}: ModelSettingsPanelProps) {
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
                  onChange={(e) => onApiKeyChange(e.target.value)}
                  onFocus={() => { if (!showKey) onShowKeyChange(true); }}
                  placeholder={t("settings.apiKeyPlaceholder")}
                  className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 pr-9 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
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
              <motion.button
                type="button"
                onClick={onSaveKey}
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
                    onClick={() => onEndpointModeChange(mode)}
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
                  onChange={(e) => onBaseUrlChange(e.target.value)}
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
                      onChange={(e) => onGenerationUrlChange(e.target.value)}
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
                        onChange={(e) => onEditUrlChange(e.target.value)}
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
                  onClick={onSaveUrl}
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
  );
}
