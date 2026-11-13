import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { Check, Eye, EyeOff, Plus, Sparkles, Trash2, X } from "lucide-react";
import type { LlmConfig } from "../../types";
import {
  useLlmConfigsQuery,
  useSaveLlmConfigsMutation,
} from "../../lib/queries/llm";
import ConfirmDialog from "../common/ConfirmDialog";
import {
  createDefaultLlmConfig,
  defaultBaseUrlForProtocol,
} from "../../lib/llmConfigDefaults";
import { canEnableLlmConfig } from "../../lib/llmConfigRules";

function maskKey(key: string): string {
  if (key.length <= 8) return "sk-****";
  return key.slice(0, 3) + "..." + key.slice(-4);
}

function protocolLabel(protocol: string): string {
  return protocol === "anthropic" ? "Anthropic" : "OpenAI";
}

function capabilityLabel(
  capability: string,
  t: (key: string) => string,
): string {
  return capability === "multimodal"
    ? t("settings.llm.capabilityMultimodal")
    : t("settings.llm.capabilityText");
}

// ── Inline edit / create form ──────────────────────────────────────────────────

interface ConfigFormProps {
  config: LlmConfig;
  showKey: boolean;
  onToggleShowKey: () => void;
  onChange: (config: LlmConfig) => void;
}

function ConfigForm({
  config,
  showKey,
  onToggleShowKey,
  onChange,
}: ConfigFormProps) {
  const { t } = useTranslation();
  const displayKey = showKey
    ? config.api_key
    : config.api_key
      ? maskKey(config.api_key)
      : "";

  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.18 }}
      className="overflow-hidden"
    >
      <div className="grid gap-3">
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formName")}
            </span>
            <input
              type="text"
              value={config.name}
              onChange={(e) => onChange({ ...config, name: e.target.value })}
              placeholder="e.g. GPT-4o"
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm placeholder:text-muted"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formProtocol")}
            </span>
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
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm"
            >
              <option value="openai">{t("settings.llm.protocolOpenai")}</option>
              <option value="anthropic">
                {t("settings.llm.protocolAnthropic")}
              </option>
            </select>
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formModel")}
            </span>
            <input
              type="text"
              value={config.model}
              onChange={(e) => onChange({ ...config, model: e.target.value })}
              placeholder={
                config.protocol === "anthropic" ? "claude-opus-4-7" : "gpt-4o"
              }
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm placeholder:text-muted"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formApiKey")}
            </span>
            <div className="relative min-w-0">
              <input
                type={showKey ? "text" : "password"}
                value={displayKey}
                onChange={(e) =>
                  onChange({ ...config, api_key: e.target.value })
                }
                onFocus={() => {
                  if (!showKey) onToggleShowKey();
                }}
                onBlur={() => {
                  if (showKey) onToggleShowKey();
                }}
                placeholder="sk-..."
                className="studio-input focus-ring h-9 w-full rounded-lg px-3 pr-9 text-sm placeholder:text-muted"
              />
              <button
                type="button"
                onClick={onToggleShowKey}
                title={
                  showKey
                    ? t("settings.llm.hideKey")
                    : t("settings.llm.showKey")
                }
                aria-label={
                  showKey
                    ? t("settings.llm.hideKey")
                    : t("settings.llm.showKey")
                }
                className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-muted transition-colors hover:bg-subtle hover:text-muted"
              >
                {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
              </button>
            </div>
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formBaseUrl")}
            </span>
            <input
              type="text"
              value={config.base_url}
              onChange={(e) =>
                onChange({ ...config, base_url: e.target.value })
              }
              placeholder={defaultBaseUrlForProtocol(config.protocol)}
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm placeholder:text-muted"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formCapability")}
            </span>
            <select
              value={config.capability}
              onChange={(e) =>
                onChange({
                  ...config,
                  capability: e.target.value as LlmConfig["capability"],
                })
              }
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm"
            >
              <option value="text">{t("settings.llm.capabilityText")}</option>
              <option value="multimodal">
                {t("settings.llm.capabilityMultimodal")}
              </option>
            </select>
          </label>
        </div>
      </div>
    </motion.div>
  );
}

// ── Single config card (collapsed) ─────────────────────────────────────────────

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
  const [deleteTargetId, setDeleteTargetId] = useState<string | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  // Sync server configs to local state when loaded
  useEffect(() => {
    if (serverConfigs) {
      setConfigs(serverConfigs);
      if (!editingId && serverConfigs[0]) {
        setEditingId(serverConfigs[0].id);
      }
    }
  }, [editingId, serverConfigs]);

  // ── Helpers ────────────────────────────────────────────────────────────────

  async function doSave(updated: LlmConfig[], preserveDraft = false) {
    const previousConfigs = configs;
    setSaveError(null);
    try {
      setConfigs(updated);
      await saveMutation.mutateAsync(updated);
      return true;
    } catch {
      setSaveError(t("settings.llm.saveError"));
      if (!preserveDraft) setConfigs(previousConfigs);
      return false;
    }
  }

  function handleAdd() {
    setShowNewForm(true);
    setEditingId(null);
  }

  function handleCancelNew() {
    setShowNewForm(false);
  }

  async function handleCreate(newConfig: LlmConfig) {
    const updated = [
      ...configs.filter((config) => config.id !== newConfig.id),
      newConfig,
    ];
    if (await doSave(updated)) setShowNewForm(false);
  }

  function handleEdit(id: string) {
    setEditingId(id);
    setShowNewForm(false);
  }

  function handleCancelEdit() {
    setEditingId(null);
    setShowKeys(new Set());
  }

  async function handleSaveEdit(updatedConfig: LlmConfig) {
    const updated = configs.map((c) =>
      c.id === updatedConfig.id ? updatedConfig : c,
    );
    if (!(await doSave(updated, true))) return;
    setEditingId(null);
    setShowKeys((prev) => {
      const next = new Set(prev);
      next.delete(updatedConfig.id);
      return next;
    });
  }

  async function handleToggleEnable(id: string) {
    const target = configs.find((config) => config.id === id);
    if (!target) return;

    if (!target.enabled) {
      const canEnable = canEnableLlmConfig(configs, id);
      if (!canEnable.ok) {
        setSaveError(
          canEnable.reason === "text_limit"
            ? t("settings.llm.enableTextLimit")
            : canEnable.reason === "multimodal_limit"
              ? t("settings.llm.enableMultimodalLimit")
              : t("settings.llm.enableCombinationLimit"),
        );
        return;
      }
    }

    const updated = configs.map((config) =>
      config.id === id ? { ...config, enabled: !target.enabled } : config,
    );
    if (!(await doSave(updated))) return;
    setEditingId(null);
    setShowKeys(new Set());
  }

  function handleDeleteRequest(id: string) {
    setDeleteTargetId(id);
  }

  async function handleConfirmDelete() {
    if (!deleteTargetId) return;
    const target = configs.find((c) => c.id === deleteTargetId);
    if (!target) {
      setDeleteTargetId(null);
      return;
    }

    setDeleteLoading(true);
    setDeleteError(null);

    const updated = configs.filter((c) => c.id !== deleteTargetId);
    setSaveError(null);
    try {
      setConfigs(updated);
      await saveMutation.mutateAsync(updated);
      if (editingId === deleteTargetId) setEditingId(null);
      setDeleteTargetId(null);
    } catch {
      setDeleteError(t("settings.llm.saveError"));
      if (serverConfigs) {
        setConfigs(serverConfigs);
      }
    } finally {
      setDeleteLoading(false);
    }
  }

  function handleCancelDelete() {
    setDeleteTargetId(null);
    setDeleteError(null);
  }

  function handleDelete(id: string) {
    const config = configs.find((c) => c.id === id);
    if (!config) return;
    handleDeleteRequest(id);
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
  const editingConfig = editingId
    ? configs.find((c) => c.id === editingId)
    : null;
  const selectedConfig = editingConfig ?? configs[0] ?? null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, x: 10 }}
      transition={{ duration: 0.18 }}
      className="pt-2"
    >
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <h3 className="text-[20px] font-semibold leading-tight text-foreground">
          {t("settings.promptOptimizationConfig")}
        </h3>
        <button
          type="button"
          onClick={handleAdd}
          disabled={saveMutation.isPending}
          aria-label={t("settings.addOptimizationService")}
          className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-primary/20 bg-primary/10 px-3 text-sm font-medium text-primary transition-colors hover:border-primary/35 hover:bg-primary/15"
        >
          <Plus size={13} />
          {t("settings.addOptimizationService")}
        </button>
      </div>

      {saveMutation.isPending ? (
        <p role="status" className="mb-3 text-sm text-muted">
          {t("studio.settings.saving", { defaultValue: "Saving changes…" })}
        </p>
      ) : null}
      {saveError && (
        <div
          role="alert"
          className="mb-4 rounded-lg border border-error/20 bg-error/5 px-4 py-3 text-xs text-error"
        >
          {saveError}
        </div>
      )}

      <fieldset
        disabled={saveMutation.isPending}
        aria-busy={saveMutation.isPending}
        className="min-w-0 border-0 p-0"
      >
        <div className="grid gap-4 xl:grid-cols-[240px_minmax(0,1fr)] xl:items-stretch">
          <section className="space-y-3 xl:border-r xl:border-border-subtle xl:pr-4">
            <div>
              <h4 className="text-sm font-semibold text-foreground">
                {t("settings.optimizationService")}
              </h4>
              <p className="mt-1 text-xs leading-relaxed text-muted">
                {t("settings.optimizationServiceDesc")}
              </p>
            </div>

            {isLoading && (
              <div className="rounded-lg border border-border-subtle bg-surface px-3 py-6 text-center text-xs text-muted">
                {t("settings.llm.loading")}
              </div>
            )}

            {isError && (
              <div className="rounded-lg border border-error/20 bg-error/5 px-3 py-3 text-xs text-error">
                {t("settings.llm.loadError")}
              </div>
            )}

            {!isLoading && !isError && !hasConfigs && !showNewForm && (
              <div className="rounded-lg border border-dashed border-border-subtle bg-surface px-3 py-6 text-center">
                <Sparkles
                  size={18}
                  className="mx-auto text-muted"
                  strokeWidth={1.5}
                />
                <p className="mt-2 text-xs text-muted">
                  {t("settings.llm.emptyTitle")}
                </p>
              </div>
            )}

            {configs.map((config) => {
              const selected = config.id === selectedConfig?.id && !showNewForm;

              return (
                <button
                  key={config.id}
                  type="button"
                  disabled={saveMutation.isPending}
                  aria-label={`Select ${config.name || t("settings.llm.untitled")} optimization service`}
                  aria-pressed={selected}
                  onClick={() => handleEdit(config.id)}
                  className={`focus-ring w-full rounded-lg border p-3 text-left transition-colors disabled:cursor-wait ${
                    selected
                      ? "border-primary/35 bg-primary/6 "
                      : "border-border-subtle bg-surface hover:border-border hover:bg-subtle/25"
                  }`}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <p className="truncate text-xs font-semibold text-foreground">
                        {config.name || t("settings.llm.untitled")}
                      </p>
                      <p className="mt-1 truncate text-xs text-muted">
                        {protocolLabel(config.protocol)} ·{" "}
                        {config.model || t("settings.llm.noModel")}
                      </p>
                    </div>
                    <span
                      className={`shrink-0 rounded-[6px] px-1.5 py-0.5 text-xs font-medium ${
                        config.enabled
                          ? "bg-success/10 text-success"
                          : "bg-subtle text-muted"
                      }`}
                    >
                      {config.enabled
                        ? t("settings.llm.enabled")
                        : t("settings.llm.disabled")}
                    </span>
                  </div>
                </button>
              );
            })}
          </section>

          <section className="min-w-0">
            {showNewForm ? (
              <NewConfigForm
                showKey={false}
                onToggleShowKey={() => {}}
                onSave={handleCreate}
                onCancel={handleCancelNew}
              />
            ) : selectedConfig ? (
              <div className="space-y-3">
                <div>
                  <h4 className="text-sm font-semibold text-foreground">
                    {selectedConfig.name || t("settings.llm.untitled")}
                  </h4>
                  <p className="mt-0.5 text-xs leading-relaxed text-muted">
                    {t("settings.promptOptimizationEditorDesc")}
                  </p>
                </div>

                <ConfigForm
                  config={selectedConfig}
                  showKey={showKeys.has(selectedConfig.id)}
                  onToggleShowKey={() => toggleShowKey(selectedConfig.id)}
                  onChange={updateEditingConfig}
                />

                <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border-subtle pt-3">
                  <p className="text-xs text-muted">
                    {selectedConfig.name || t("settings.llm.untitled")} ·{" "}
                    {protocolLabel(selectedConfig.protocol)}
                  </p>
                  <div className="flex flex-wrap justify-end gap-2">
                    <motion.button
                      type="button"
                      onClick={() => handleToggleEnable(selectedConfig.id)}
                                className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-accent/20 bg-accent/10 px-4 text-sm font-medium text-accent transition-colors hover:border-accent/35 hover:bg-accent/15 lg:min-w-[104px]"
                    >
                      <Check size={13} />
                      {selectedConfig.enabled
                        ? t("settings.llm.deactivate")
                        : t("settings.llm.activate")}
                    </motion.button>
                    <motion.button
                      type="button"
                      onClick={() => handleSaveEdit(selectedConfig)}
                                className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-accent/20 bg-accent/10 px-4 text-sm font-medium text-accent transition-colors hover:border-accent/35 hover:bg-accent/15 lg:min-w-[104px]"
                    >
                      {t("settings.llm.saveConfig")}
                    </motion.button>
                    <motion.button
                      type="button"
                      onClick={handleCancelEdit}
                                className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-border-subtle bg-surface px-4 text-sm font-medium text-muted transition-colors hover:border-border hover:text-foreground lg:min-w-[104px]"
                    >
                      <X size={13} />
                      {t("settings.cancelEdit")}
                    </motion.button>
                    <motion.button
                      type="button"
                      onClick={() => handleDelete(selectedConfig.id)}
                                className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-error/20 bg-error/5 px-4 text-sm font-medium text-error transition-colors hover:border-error/30 hover:bg-error/10"
                    >
                      <Trash2 size={13} />
                      {t("settings.llm.deleteTitle")}
                    </motion.button>
                  </div>
                </div>

                <div className="grid gap-3 border-t border-border-subtle pt-3 sm:grid-cols-2">
                  <div className="rounded-lg border border-dashed border-border-subtle bg-surface p-3">
                    <p className="text-xs text-muted">
                      {t("settings.promptOptimizationUsageLabel")}
                    </p>
                    <p className="mt-1 text-sm font-semibold text-foreground">
                      {t("settings.promptOptimizationUsageTitle")}
                    </p>
                    <p className="mt-1 text-xs leading-relaxed text-muted">
                      {t("settings.promptOptimizationUsageHint")}
                    </p>
                  </div>
                  <div className="rounded-lg border border-dashed border-border-subtle bg-surface p-3">
                    <p className="text-xs text-muted">
                      {t("settings.promptOptimizationProtocolLabel")}
                    </p>
                    <p className="mt-1 text-sm font-semibold text-foreground">
                      {protocolLabel(selectedConfig.protocol)}
                    </p>
                    <p className="mt-1 text-xs leading-relaxed text-muted">
                      {capabilityLabel(selectedConfig.capability, t)}
                    </p>
                  </div>
                </div>
              </div>
            ) : (
              <div className="flex h-full min-h-[220px] flex-col items-center justify-center rounded-[12px] border border-dashed border-border-subtle bg-surface text-center">
                <Sparkles size={22} className="text-muted" strokeWidth={1.5} />
                <p className="mt-3 text-xs text-muted">
                  {t("settings.llm.emptyHint")}
                </p>
              </div>
            )}
          </section>
        </div>
      </fieldset>
      <ConfirmDialog
        open={deleteTargetId !== null}
        title={
          deleteTargetId
            ? t("settings.llm.deleteConfirm", {
                name:
                  configs.find((config) => config.id === deleteTargetId)
                    ?.name || t("settings.llm.untitled"),
              })
            : t("settings.llm.deleteTitle")
        }
        confirmLabel={t("settings.llm.deleteTitle")}
        cancelLabel={t("settings.cancelEdit")}
        onConfirm={handleConfirmDelete}
        onCancel={handleCancelDelete}
        loading={deleteLoading}
        error={deleteError}
      />
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

  const displayKey = showKey
    ? config.api_key
    : config.api_key
      ? maskKey(config.api_key)
      : "";

  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.18 }}
      className="overflow-hidden"
    >
      <div className="rounded-[12px] border border-primary/20 bg-primary/3 p-4">
        <div className="flex items-center gap-2 mb-3">
          <span className="text-xs font-semibold text-foreground">
            {t("settings.llm.newTitle")}
          </span>
          <span className="rounded-[6px] border border-primary/15 bg-primary/8 px-1.5 py-0.5 text-xs font-medium text-primary">
            {t("settings.llm.newBadge")}
          </span>
        </div>

        <div className="grid gap-2 sm:grid-cols-2">
          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formName")}
            </span>
            <input
              type="text"
              value={config.name}
              onChange={(e) => setConfig({ ...config, name: e.target.value })}
              placeholder="e.g. GPT-4o"
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm placeholder:text-muted"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formProtocol")}
            </span>
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
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm"
            >
              <option value="openai">{t("settings.llm.protocolOpenai")}</option>
              <option value="anthropic">
                {t("settings.llm.protocolAnthropic")}
              </option>
            </select>
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formModel")}
            </span>
            <input
              type="text"
              value={config.model}
              onChange={(e) => setConfig({ ...config, model: e.target.value })}
              placeholder={
                config.protocol === "anthropic" ? "claude-opus-4-7" : "gpt-4o"
              }
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm placeholder:text-muted"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formApiKey")}
            </span>
            <div className="relative min-w-0">
              <input
                type={showKey ? "text" : "password"}
                value={displayKey}
                onChange={(e) =>
                  setConfig({ ...config, api_key: e.target.value })
                }
                onFocus={() => {
                  if (!showKey) setShowKey(true);
                }}
                onBlur={() => {
                  if (showKey) setShowKey(false);
                }}
                placeholder="sk-..."
                className="studio-input focus-ring h-9 w-full rounded-lg px-3 pr-9 text-sm placeholder:text-muted"
              />
              <button
                type="button"
                onClick={() => setShowKey(!showKey)}
                title={
                  showKey
                    ? t("settings.llm.hideKey")
                    : t("settings.llm.showKey")
                }
                aria-label={
                  showKey
                    ? t("settings.llm.hideKey")
                    : t("settings.llm.showKey")
                }
                className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-muted transition-colors hover:bg-subtle hover:text-muted"
              >
                {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
              </button>
            </div>
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formBaseUrl")}
            </span>
            <input
              type="text"
              value={config.base_url}
              onChange={(e) =>
                setConfig({ ...config, base_url: e.target.value })
              }
              placeholder={defaultBaseUrlForProtocol(config.protocol)}
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm placeholder:text-muted"
            />
          </label>

          <label className="grid gap-1.5">
            <span className="text-xs font-medium text-muted">
              {t("settings.llm.formCapability")}
            </span>
            <select
              value={config.capability}
              onChange={(e) =>
                setConfig({
                  ...config,
                  capability: e.target.value as LlmConfig["capability"],
                })
              }
              className="studio-input focus-ring h-9 w-full rounded-lg px-3 text-sm"
            >
              <option value="text">{t("settings.llm.capabilityText")}</option>
              <option value="multimodal">
                {t("settings.llm.capabilityMultimodal")}
              </option>
            </select>
          </label>
        </div>

        {/* Actions */}
        <div className="flex items-center justify-end gap-2 mt-3 pt-3 border-t border-border-subtle">
          <motion.button
            type="button"
            onClick={onCancel}
            className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-border-subtle bg-surface px-3 text-sm font-medium text-muted transition-colors hover:border-border hover:text-foreground"
          >
            <X size={13} />
            {t("settings.llm.cancel")}
          </motion.button>
          <motion.button
            type="button"
            onClick={() => onSave(config)}
            className="flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-accent/25 bg-accent/10 px-3 text-sm font-medium text-accent transition-colors hover:border-accent/40 hover:bg-accent/15"
          >
            <Check size={13} />
            {t("settings.llm.create")}
          </motion.button>
        </div>
      </div>
    </motion.div>
  );
}
