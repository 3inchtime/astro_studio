import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import {
  Check,
  Eye,
  EyeOff,
  Plus,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import type { LlmConfig } from "../../types";
import { useLlmConfigsQuery, useSaveLlmConfigsMutation } from "../../lib/queries/llm";
import {
  createDefaultLlmConfig,
  defaultBaseUrlForProtocol,
} from "../../lib/llmConfigDefaults";
import { cardVariants, sectionVariants } from "./settingsMotion";

function maskKey(key: string): string {
  if (key.length <= 8) return "sk-****";
  return key.slice(0, 3) + "..." + key.slice(-4);
}

function protocolLabel(protocol: string): string {
  return protocol === "anthropic" ? "Anthropic" : "OpenAI";
}

function capabilityLabel(capability: string, t: (key: string) => string): string {
  return capability === "multimodal" ? t("settings.llm.capabilityMultimodal") : t("settings.llm.capabilityText");
}

// ── Inline edit / create form ──────────────────────────────────────────────────

interface ConfigFormProps {
  config: LlmConfig;
  showKey: boolean;
  onToggleShowKey: () => void;
  onChange: (config: LlmConfig) => void;
  onSave: () => void;
  onCancel: () => void;
  isNew?: boolean;
}

function ConfigForm({
  config,
  showKey,
  onToggleShowKey,
  onChange,
  onSave,
  onCancel,
  isNew,
}: ConfigFormProps) {
  const { t } = useTranslation();
  const displayKey =
    showKey ? config.api_key : config.api_key ? maskKey(config.api_key) : "";

  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.2 }}
      className="overflow-hidden"
    >
      <div className="grid gap-3 rounded-[12px] border border-border-subtle bg-subtle/20 p-4">
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formName")}</span>
            <input
              type="text"
              value={config.name}
              onChange={(e) => onChange({ ...config, name: e.target.value })}
              placeholder="e.g. GPT-4o"
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formProtocol")}</span>
            <select
              value={config.protocol}
              onChange={(e) => {
                const next = e.target.value as LlmConfig["protocol"];
                onChange({
                  ...config,
                  protocol: next,
                  base_url: defaultBaseUrlForProtocol(next),
                });
              }}
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            >
              <option value="openai">{t("settings.llm.protocolOpenai")}</option>
              <option value="anthropic">{t("settings.llm.protocolAnthropic")}</option>
            </select>
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formModel")}</span>
            <input
              type="text"
              value={config.model}
              onChange={(e) => onChange({ ...config, model: e.target.value })}
              placeholder={config.protocol === "anthropic" ? "claude-opus-4-7" : "gpt-4o"}
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formApiKey")}</span>
            <div className="relative min-w-0">
              <input
                type={showKey ? "text" : "password"}
                value={displayKey}
                onChange={(e) => onChange({ ...config, api_key: e.target.value })}
                placeholder="sk-..."
                className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 pr-9 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
              />
              <button
                type="button"
                onClick={onToggleShowKey}
                title={showKey ? t("settings.llm.hideKey") : t("settings.llm.showKey")}
                aria-label={showKey ? t("settings.llm.hideKey") : t("settings.llm.showKey")}
                className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-muted/40 transition-colors hover:bg-subtle hover:text-muted"
              >
                {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
              </button>
            </div>
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formBaseUrl")}</span>
            <input
              type="text"
              value={config.base_url}
              onChange={(e) => onChange({ ...config, base_url: e.target.value })}
              placeholder={defaultBaseUrlForProtocol(config.protocol)}
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formCapability")}</span>
            <select
              value={config.capability}
              onChange={(e) =>
                onChange({ ...config, capability: e.target.value as LlmConfig["capability"] })
              }
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            >
              <option value="text">{t("settings.llm.capabilityText")}</option>
              <option value="multimodal">{t("settings.llm.capabilityMultimodal")}</option>
            </select>
          </label>
        </div>

        {/* Enabled toggle */}
        <label className="flex items-center gap-2.5 select-none">
          <button
            type="button"
            role="switch"
            aria-checked={config.enabled}
            onClick={() => onChange({ ...config, enabled: !config.enabled })}
            className={`relative inline-flex h-[22px] w-[40px] shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 ${
              config.enabled
                ? "bg-primary"
                : "bg-border"
            }`}
          >
            <span
              className={`inline-block h-[16px] w-[16px] transform rounded-full bg-white shadow-sm transition-transform duration-200 ${
                config.enabled ? "translate-x-[20px]" : "translate-x-[3px]"
              }`}
            />
          </button>
          <span className="text-[12px] font-medium text-muted/70">{t("settings.llm.enabled")}</span>
        </label>

        {/* Actions */}
        <div className="flex items-center justify-end gap-2 pt-1 border-t border-border-subtle">
          <motion.button
            type="button"
            onClick={onCancel}
            whileTap={{ scale: 0.97 }}
            className="flex h-[34px] shrink-0 items-center justify-center gap-1.5 rounded-[9px] border border-border-subtle bg-surface px-3 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
          >
            <X size={13} />
            {t("settings.llm.cancel")}
          </motion.button>
          <motion.button
            type="button"
            onClick={onSave}
            whileTap={{ scale: 0.97 }}
            className="flex h-[34px] shrink-0 items-center justify-center gap-1.5 rounded-[9px] border border-primary/25 bg-primary/10 px-3 text-[12px] font-medium text-primary transition-all hover:border-primary/40 hover:bg-primary/15"
          >
            <Check size={13} />
            {isNew ? t("settings.llm.create") : t("settings.llm.save")}
          </motion.button>
        </div>
      </div>
    </motion.div>
  );
}

// ── Single config card (collapsed) ─────────────────────────────────────────────

interface ConfigCardProps {
  config: LlmConfig;
  onEdit: () => void;
  onDelete: () => void;
  onToggleEnabled: () => void;
}

function ConfigCard({
  config,
  onEdit,
  onDelete,
  onToggleEnabled,
}: ConfigCardProps) {
  const { t } = useTranslation();
  return (
    <div className="rounded-[12px] border border-border-subtle bg-surface shadow-card p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[13px] font-semibold text-foreground">
              {config.name || t("settings.llm.untitled")}
            </span>
            <span className={`rounded-[6px] border px-1.5 py-0.5 text-[10px] font-medium uppercase ${
              config.protocol === "anthropic"
                ? "border-amber-200/50 bg-amber-50 text-amber-700"
                : "border-emerald-200/50 bg-emerald-50 text-emerald-700"
            }`}>
              {protocolLabel(config.protocol)}
            </span>
          </div>
          <p className="mt-1 truncate font-mono text-[10.5px] text-muted/55">
            {config.model || t("settings.llm.noModel")}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <span className="rounded-[6px] border border-border-subtle bg-subtle/40 px-1.5 py-0.5 text-[10.5px] font-medium text-muted/65">
            {capabilityLabel(config.capability, t)}
          </span>
        </div>
      </div>

      <div className="mt-3 flex flex-wrap items-center justify-between gap-3 border-t border-border-subtle pt-3">
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 select-none">
            <button
              type="button"
              role="switch"
              aria-checked={config.enabled}
              onClick={onToggleEnabled}
              className={`relative inline-flex h-[22px] w-[40px] shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 ${
                config.enabled
                  ? "bg-primary"
                  : "bg-border"
              }`}
            >
              <span
                className={`inline-block h-[16px] w-[16px] transform rounded-full bg-white shadow-sm transition-transform duration-200 ${
                  config.enabled ? "translate-x-[20px]" : "translate-x-[3px]"
                }`}
              />
            </button>
            <span className="text-[11px] text-muted/60">
              {config.enabled ? t("settings.llm.enabled") : t("settings.llm.disabled")}
            </span>
          </label>
        </div>
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            onClick={onEdit}
            className="flex h-[30px] items-center justify-center rounded-[8px] border border-border-subtle bg-surface px-3 text-[11px] font-medium text-muted/65 transition-all hover:border-border hover:text-foreground"
          >
            {t("settings.llm.edit")}
          </button>
          <button
            type="button"
            onClick={onDelete}
            title={t("settings.llm.deleteTitle")}
            aria-label={t("settings.llm.deleteTitle")}
            className="flex h-[30px] w-[30px] items-center justify-center rounded-[8px] border border-border-subtle bg-surface text-muted/55 transition-all hover:border-border hover:text-foreground"
          >
            <Trash2 size={13} />
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Section ────────────────────────────────────────────────────────────────────

export function LlmConfigSection() {
  const { t } = useTranslation();
  const { data: serverConfigs, isLoading, isError } = useLlmConfigsQuery();
  const saveMutation = useSaveLlmConfigsMutation();

  const [configs, setConfigs] = useState<LlmConfig[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showNewForm, setShowNewForm] = useState(false);
  const [showKeys, setShowKeys] = useState<Set<string>>(new Set());
  const [saveError, setSaveError] = useState<string | null>(null);

  // Sync server configs to local state when loaded
  useEffect(() => {
    if (serverConfigs) {
      setConfigs(serverConfigs);
    }
  }, [serverConfigs]);

  // ── Helpers ────────────────────────────────────────────────────────────────

  async function doSave(updated: LlmConfig[]) {
    setSaveError(null);
    try {
      setConfigs(updated);
      await saveMutation.mutateAsync(updated);
    } catch {
      setSaveError(t("settings.llm.saveError"));
      // Revert on save failure
      if (serverConfigs) {
        setConfigs(serverConfigs);
      }
    }
  }

  function handleAdd() {
    setShowNewForm(true);
    setEditingId(null);
  }

  function handleCancelNew() {
    setShowNewForm(false);
  }

  function handleCreate(newConfig: LlmConfig) {
    const updated = [...configs, newConfig];
    doSave(updated);
    setShowNewForm(false);
  }

  function handleEdit(id: string) {
    setEditingId(id);
    setShowNewForm(false);
  }

  function handleCancelEdit() {
    setEditingId(null);
  }

  function handleSaveEdit(updatedConfig: LlmConfig) {
    const updated = configs.map((c) =>
      c.id === updatedConfig.id ? updatedConfig : c,
    );
    doSave(updated);
    setEditingId(null);
  }

  function handleDelete(id: string) {
    const config = configs.find((c) => c.id === id);
    if (!config) return;
    const name = config.name || t("settings.llm.untitled");
    if (!window.confirm(t("settings.llm.deleteConfirm", { name }))) return;
    const updated = configs.filter((c) => c.id !== id);
    doSave(updated);
    if (editingId === id) setEditingId(null);
  }

  function handleToggleEnabled(id: string) {
    const updated = configs.map((c) =>
      c.id === id ? { ...c, enabled: !c.enabled } : c,
    );
    doSave(updated);
  }

  function toggleShowKey(id: string) {
    setShowKeys((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function updateEditingConfig(patch: LlmConfig) {
    setConfigs((prev) => prev.map((c) => (c.id === patch.id ? patch : c)));
  }

  // ── Derived state ──────────────────────────────────────────────────────────

  const hasConfigs = configs.length > 0;
  const editingConfig = editingId ? configs.find((c) => c.id === editingId) : null;

  return (
    <motion.div
      initial={{ opacity: 0, x: 10 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: 10 }}
      transition={{ duration: 0.2, delay: 0.1 }}
    >
      <motion.section
        custom={2}
        variants={sectionVariants}
        initial="hidden"
        animate="visible"
        className="space-y-3"
      >
        <div className="flex items-center gap-2">
          <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[8px] border border-primary/10 bg-primary/5">
            <Sparkles size={14} className="text-primary" strokeWidth={2} />
          </div>
          <div>
            <h3 className="text-[13px] font-semibold text-foreground">{t("settings.llm.title")}</h3>
            <p className="mt-0.5 text-[11px] text-muted/60">
              {t("settings.llm.desc")}
            </p>
          </div>
        </div>

        <motion.div
          custom={2}
          variants={cardVariants}
          initial="hidden"
          animate="visible"
          className="rounded-[12px] border border-border-subtle bg-surface shadow-card"
        >
          <div className="p-5 space-y-4">
            {/* Error state */}
            {saveError && (
              <div className="rounded-[10px] border border-error/20 bg-error/5 px-4 py-3 text-[12px] text-error">
                {saveError}
              </div>
            )}

            {/* Loading state */}
            {isLoading && (
              <div className="flex items-center justify-center py-8 text-[12px] text-muted/60">
                {t("settings.llm.loading")}
              </div>
            )}

            {/* Error loading */}
            {isError && (
              <div className="rounded-[10px] border border-error/20 bg-error/5 px-4 py-3 text-[12px] text-error">
                {t("settings.llm.loadError")}
              </div>
            )}

            {/* Empty state */}
            {!isLoading && !isError && !hasConfigs && !showNewForm && (
              <div className="flex flex-col items-center justify-center py-10 space-y-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-[12px] border border-border-subtle bg-subtle/30">
                  <Sparkles size={20} className="text-muted/40" strokeWidth={1.5} />
                </div>
                <p className="text-[12px] text-muted/60">{t("settings.llm.emptyTitle")}</p>
                <p className="text-[11px] text-muted/40">
                  {t("settings.llm.emptyHint")}
                </p>
              </div>
            )}

            {/* Config list */}
            {hasConfigs && (
              <div className="space-y-3">
                {configs.map((config) => (
                  <div key={config.id} className="space-y-3">
                    <ConfigCard
                      config={config}
                      onEdit={() => handleEdit(config.id)}
                      onDelete={() => handleDelete(config.id)}
                      onToggleEnabled={() => handleToggleEnabled(config.id)}
                    />

                    {/* Edit form */}
                    {editingId === config.id && editingConfig && (
                      <ConfigForm
                        config={editingConfig}
                        showKey={showKeys.has(config.id)}
                        onToggleShowKey={() => toggleShowKey(config.id)}
                        onChange={updateEditingConfig}
                        onSave={() => handleSaveEdit(editingConfig)}
                        onCancel={handleCancelEdit}
                      />
                    )}
                  </div>
                ))}
              </div>
            )}

            {/* New config form */}
            {showNewForm && (
              <NewConfigForm
                showKey={false}
                onToggleShowKey={() => {}}
                onSave={handleCreate}
                onCancel={handleCancelNew}
              />
            )}

            {/* Add button */}
            {!showNewForm && (
              <button
                type="button"
                onClick={handleAdd}
                className="flex h-[38px] w-full items-center justify-center gap-1.5 rounded-[10px] border border-dashed border-border-subtle bg-subtle/15 px-3 text-[12px] font-medium text-muted transition-all hover:border-border hover:bg-subtle/30 hover:text-foreground"
              >
                <Plus size={13} />
                {t("settings.llm.add")}
              </button>
            )}
          </div>
        </motion.div>
      </motion.section>
    </motion.div>
  );
}

// ── New config form (standalone, not attached to existing cards) ───────────────

interface NewConfigFormProps {
  showKey: boolean;
  onToggleShowKey: () => void;
  onSave: (config: LlmConfig) => void;
  onCancel: () => void;
}

function NewConfigForm({ onSave, onCancel }: NewConfigFormProps) {
  const { t } = useTranslation();
  const [config, setConfig] = useState<LlmConfig>(createDefaultLlmConfig());
  const [showKey, setShowKey] = useState(false);

  const displayKey =
    showKey ? config.api_key : config.api_key ? maskKey(config.api_key) : "";

  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.2 }}
      className="overflow-hidden"
    >
      <div className="rounded-[12px] border border-primary/20 bg-primary/3 p-4">
        <div className="flex items-center gap-2 mb-3">
          <span className="text-[12px] font-semibold text-foreground">{t("settings.llm.newTitle")}</span>
          <span className="rounded-[6px] border border-primary/15 bg-primary/8 px-1.5 py-0.5 text-[10px] font-medium text-primary">
            {t("settings.llm.newBadge")}
          </span>
        </div>

        <div className="grid gap-2 sm:grid-cols-2">
          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formName")}</span>
            <input
              type="text"
              value={config.name}
              onChange={(e) => setConfig({ ...config, name: e.target.value })}
              placeholder="e.g. GPT-4o"
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formProtocol")}</span>
            <select
              value={config.protocol}
              onChange={(e) => {
                const next = e.target.value as LlmConfig["protocol"];
                setConfig({
                  ...config,
                  protocol: next,
                  base_url: defaultBaseUrlForProtocol(next),
                });
              }}
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            >
              <option value="openai">{t("settings.llm.protocolOpenai")}</option>
              <option value="anthropic">{t("settings.llm.protocolAnthropic")}</option>
            </select>
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formModel")}</span>
            <input
              type="text"
              value={config.model}
              onChange={(e) => setConfig({ ...config, model: e.target.value })}
              placeholder={config.protocol === "anthropic" ? "claude-opus-4-7" : "gpt-4o"}
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formApiKey")}</span>
            <div className="relative min-w-0">
              <input
                type={showKey ? "text" : "password"}
                value={displayKey}
                onChange={(e) => setConfig({ ...config, api_key: e.target.value })}
                placeholder="sk-..."
                className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 pr-9 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
              />
              <button
                type="button"
                onClick={() => setShowKey(!showKey)}
                title={showKey ? t("settings.llm.hideKey") : t("settings.llm.showKey")}
                aria-label={showKey ? t("settings.llm.hideKey") : t("settings.llm.showKey")}
                className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-muted/40 transition-colors hover:bg-subtle hover:text-muted"
              >
                {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
              </button>
            </div>
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formBaseUrl")}</span>
            <input
              type="text"
              value={config.base_url}
              onChange={(e) => setConfig({ ...config, base_url: e.target.value })}
              placeholder={defaultBaseUrlForProtocol(config.protocol)}
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.llm.formCapability")}</span>
            <select
              value={config.capability}
              onChange={(e) =>
                setConfig({ ...config, capability: e.target.value as LlmConfig["capability"] })
              }
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground transition-all duration-200 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            >
              <option value="text">{t("settings.llm.capabilityText")}</option>
              <option value="multimodal">{t("settings.llm.capabilityMultimodal")}</option>
            </select>
          </label>
        </div>

        {/* Enabled toggle */}
        <label className="flex items-center gap-2.5 select-none mt-3">
          <button
            type="button"
            role="switch"
            aria-checked={config.enabled}
            onClick={() => setConfig({ ...config, enabled: !config.enabled })}
            className={`relative inline-flex h-[22px] w-[40px] shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 ${
              config.enabled
                ? "bg-primary"
                : "bg-border"
            }`}
          >
            <span
              className={`inline-block h-[16px] w-[16px] transform rounded-full bg-white shadow-sm transition-transform duration-200 ${
                config.enabled ? "translate-x-[20px]" : "translate-x-[3px]"
              }`}
            />
          </button>
          <span className="text-[12px] font-medium text-muted/70">{t("settings.llm.enabled")}</span>
        </label>

        {/* Actions */}
        <div className="flex items-center justify-end gap-2 mt-3 pt-3 border-t border-border-subtle">
          <motion.button
            type="button"
            onClick={onCancel}
            whileTap={{ scale: 0.97 }}
            className="flex h-[34px] shrink-0 items-center justify-center gap-1.5 rounded-[9px] border border-border-subtle bg-surface px-3 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
          >
            <X size={13} />
            {t("settings.llm.cancel")}
          </motion.button>
          <motion.button
            type="button"
            onClick={() => onSave(config)}
            whileTap={{ scale: 0.97 }}
            className="flex h-[34px] shrink-0 items-center justify-center gap-1.5 rounded-[9px] border border-primary/25 bg-primary/10 px-3 text-[12px] font-medium text-primary transition-all hover:border-primary/40 hover:bg-primary/15"
          >
            <Check size={13} />
            {t("settings.llm.create")}
          </motion.button>
        </div>
      </div>
    </motion.div>
  );
}
