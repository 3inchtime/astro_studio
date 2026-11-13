import { motion } from "framer-motion";
import { Check, Download } from "lucide-react";
import type { TFunction } from "i18next";
import type { MouseEvent as ReactMouseEvent } from "react";
import type { AppFontSize, TrashSettings } from "../../types";
import { APP_FONT_SIZE_OPTIONS } from "../../lib/fontSize";
import {
  LANGUAGE_OPTIONS,
  normalizeLanguage,
  type SupportedLanguage,
} from "../../lib/languages";
import { ThemeCardPicker } from "../theme/ThemeCardPicker";
import type { ThemeId } from "../../lib/themes";

interface GeneralSettingsPanelProps {
  t: TFunction;
  language: SupportedLanguage;
  trashSettings: TrashSettings;
  trashSaved: boolean;
  fontSize: AppFontSize;
  fontSizeSaved: boolean;
  theme: ThemeId;
  appVersion: string;
  updateSupported: boolean;
  fontSizeLabelKeys: Record<AppFontSize, string>;
  onLanguageChange: (language: SupportedLanguage) => void;
  onTrashSettingsChange: (settings: TrashSettings) => void;
  onSaveTrashRetention: () => void;
  onOpenTrash: () => void;
  onFontSizeChange: (fontSize: AppFontSize) => void;
  onThemeChange: (theme: ThemeId, event?: ReactMouseEvent<HTMLButtonElement>) => void;
  onCheckUpdate: () => void;
  updateChecking: boolean;
}

export function GeneralSettingsPanel({
  t,
  language,
  trashSettings,
  trashSaved,
  fontSize,
  fontSizeSaved,
  theme,
  appVersion,
  updateSupported,
  fontSizeLabelKeys,
  onLanguageChange,
  onTrashSettingsChange,
  onSaveTrashRetention,
  onOpenTrash,
  onFontSizeChange,
  onThemeChange,
  onCheckUpdate,
  updateChecking,
}: GeneralSettingsPanelProps) {
  return (
    <div className="space-y-6">
      <section aria-label={t("settings.general")} className="space-y-4">
        <div className="grid items-center gap-3 sm:grid-cols-[140px_minmax(0,1fr)]">
          <label htmlFor="settings-language" className="text-sm font-medium text-foreground">{t("settings.language")}</label>
          <select
            id="settings-language"
            aria-label={t("settings.language")}
            value={language}
            onChange={(event) => onLanguageChange(normalizeLanguage(event.target.value))}
            className="select-control studio-input focus-ring h-9 w-full rounded-lg px-3 pr-8 text-sm"
          >
            {LANGUAGE_OPTIONS.map((option) => <option key={option.code} value={option.code}>{option.label}</option>)}
          </select>
        </div>
        <div className="grid items-center gap-3 sm:grid-cols-[140px_minmax(0,1fr)]">
          <span className="text-sm font-medium text-foreground">{t("settings.fontSize")}</span>
          <div className="grid grid-cols-3 gap-1 rounded-lg border border-border-subtle p-1">
            {APP_FONT_SIZE_OPTIONS.map((option) => (
              <button
                key={option}
                type="button"
                aria-pressed={fontSize === option}
                onClick={() => onFontSizeChange(option)}
                className={`focus-ring flex min-h-8 items-center justify-center gap-2 rounded-md px-2 text-sm transition-colors ${fontSize === option ? "bg-surface font-medium text-foreground" : "text-muted hover:bg-surface"}`}
              >
                {t(fontSizeLabelKeys[option])}
                {fontSize === option && <Check size={13} aria-hidden="true" className="shrink-0 text-primary" />}
              </button>
            ))}
          </div>
          {fontSizeSaved && <span className="text-xs text-success sm:col-start-2">{t("settings.saved")}</span>}
        </div>
      </section>

      <section aria-labelledby="settings-theme-heading" className="border-t border-border-subtle pt-5">
        <h2 id="settings-theme-heading" className="mb-3 text-base font-semibold text-foreground">{t("settings.theme")}</h2>
        <ThemeCardPicker
          selectedThemeId={theme}
          activeLabel={t("theme.active")}
          t={t}
          compact
          gridClassName="grid-cols-3 xl:grid-cols-4"
          onSelect={(themeId, event) => onThemeChange(themeId, event)}
        />
      </section>

      <section className="border-t border-border-subtle pt-5">
        <div className="mb-3">
          <h2 className="text-base font-semibold text-foreground">{t("settings.trashRetention")}</h2>
          <p className="mt-1 text-xs leading-relaxed text-muted">{t("settings.trashRetentionDesc")}</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <label className="studio-input flex h-9 min-w-0 items-center gap-2 rounded-lg px-3">
            <input
              type="number"
              aria-label={t("settings.trashRetention")}
              min={1}
              max={365}
              value={trashSettings.retention_days}
              onChange={(event) => onTrashSettingsChange({ ...trashSettings, retention_days: Math.min(365, Math.max(1, Number(event.target.value) || 1)) })}
              className="w-16 min-w-0 bg-transparent text-sm text-foreground outline-none"
            />
            <span className="text-xs text-muted">{t("settings.days")}</span>
          </label>
          <button onClick={onSaveTrashRetention} className="studio-control-primary focus-ring inline-flex h-9 items-center justify-center gap-1.5 rounded-lg px-4 text-sm font-medium">
            {trashSaved && <Check size={13} aria-hidden="true" />}
            {trashSaved ? t("settings.saved") : t("settings.saveTrashRetention")}
          </button>
          <button type="button" onClick={onOpenTrash} className="studio-control focus-ring h-9 rounded-lg px-3 text-sm">{t("settings.openTrash")}</button>
        </div>
      </section>

      <section className="border-t border-border-subtle pt-5">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-baseline gap-3">
            <h2 className="text-base font-semibold text-foreground">{t("settings.about")}</h2>
            <span className="text-sm tabular-nums text-muted">v{appVersion}</span>
          </div>
          {updateSupported && (
            <button type="button" onClick={onCheckUpdate} disabled={updateChecking} className="studio-control focus-ring inline-flex h-9 items-center justify-center gap-2 rounded-lg px-3 text-sm font-medium disabled:opacity-50">
              {updateChecking ? <motion.span animate={{ rotate: 360 }} transition={{ duration: 1, repeat: Infinity, ease: "linear" }} className="h-3.5 w-3.5 rounded-full border-2 border-current border-t-transparent" /> : <Download size={14} />}
              {t(updateChecking ? "update.checking" : "update.check")}
            </button>
          )}
        </div>
      </section>
    </div>
  );
}
