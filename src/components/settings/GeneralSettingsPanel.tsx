import { motion } from "framer-motion";
import { Check, Languages, SlidersHorizontal, Trash2, Type } from "lucide-react";
import type { TFunction } from "i18next";
import type { AppFontSize, TrashSettings } from "../../types";
import { APP_FONT_SIZE_OPTIONS } from "../../lib/fontSize";
import {
  LANGUAGE_OPTIONS,
  normalizeLanguage,
  type SupportedLanguage,
} from "../../lib/languages";
import { cardVariants, sectionVariants } from "./settingsMotion";

interface GeneralSettingsPanelProps {
  t: TFunction;
  language: SupportedLanguage;
  trashSettings: TrashSettings;
  trashSaved: boolean;
  fontSize: AppFontSize;
  fontSizeSaved: boolean;
  fontSizeLabelKeys: Record<AppFontSize, string>;
  onLanguageChange: (language: SupportedLanguage) => void;
  onTrashSettingsChange: (settings: TrashSettings) => void;
  onSaveTrashRetention: () => void;
  onOpenTrash: () => void;
  onFontSizeChange: (fontSize: AppFontSize) => void;
}

export function GeneralSettingsPanel({
  t,
  language,
  trashSettings,
  trashSaved,
  fontSize,
  fontSizeSaved,
  fontSizeLabelKeys,
  onLanguageChange,
  onTrashSettingsChange,
  onSaveTrashRetention,
  onOpenTrash,
  onFontSizeChange,
}: GeneralSettingsPanelProps) {
  return (
    <motion.div
      key="general"
      initial={{ opacity: 0, x: -10 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: -10 }}
      transition={{ duration: 0.2 }}
    >
      <div className="space-y-6">
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
                value={language}
                onChange={(e) => onLanguageChange(normalizeLanguage(e.target.value))}
                className="select-control h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 pr-8 text-[12px] text-foreground transition-all duration-200 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
              >
                {LANGUAGE_OPTIONS.map((option) => (
                  <option key={option.code} value={option.code}>
                    {option.label}
                  </option>
                ))}
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
                    onChange={(e) => onTrashSettingsChange({
                      ...trashSettings,
                      retention_days: Math.min(365, Math.max(1, Number(e.target.value) || 1)),
                    })}
                    className="h-[38px] min-w-0 flex-1 bg-transparent text-[12px] text-foreground focus:outline-none"
                  />
                  <span className="text-[11px] text-muted/60">{t("settings.days")}</span>
                </div>
                <button
                  onClick={onSaveTrashRetention}
                  className="inline-flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] gradient-primary px-4 text-[12px] font-medium text-white shadow-button transition-transform hover:-translate-y-0.5"
                >
                  {trashSaved && <Check size={13} strokeWidth={2.5} />}
                  {trashSaved ? t("settings.saved") : t("settings.saveTrashRetention")}
                </button>
                <button
                  type="button"
                  onClick={onOpenTrash}
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
                        onClick={() => onFontSizeChange(option)}
                        className={`rounded-[10px] border px-3 py-3 text-left transition-all ${
                          active
                            ? "border-primary/30 bg-primary/6 shadow-card"
                            : "border-border-subtle bg-subtle/20 hover:border-border hover:bg-subtle/40"
                        }`}
                      >
                        <div className="flex items-center justify-between gap-3">
                          <span className="text-[12px] font-medium text-foreground">{t(fontSizeLabelKeys[option])}</span>
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
                    {fontSizeSaved ? t("settings.saved") : t(fontSizeLabelKeys[fontSize])}
                  </span>
                </div>
              </div>
            </div>
          </motion.div>
        </motion.section>
      </div>
    </motion.div>
  );
}
