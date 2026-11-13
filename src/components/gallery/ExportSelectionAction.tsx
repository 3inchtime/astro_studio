import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { GenerationResult } from "../../types";
import { useDialogFocus } from "../../hooks/useDialogFocus";
import { useExportStore } from "../../lib/exportStore";
const button =
  "studio-control focus-ring min-h-9 rounded-lg px-3 py-2 text-xs font-medium disabled:opacity-40";
export default function ExportSelectionAction({
  rows,
  selectedIds,
}: {
  rows: GenerationResult[];
  selectedIds: Set<string>;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [includeRecipe, setIncludeRecipe] = useState(false);
  const busy = useExportStore((s) => s.busy);
  const start = useExportStore((s) => s.start);
  const [targets, setTargets] = useState<GenerationResult[]>([]);
  const close = () => setOpen(false);
  const ref = useDialogFocus({ open, onClose: close });
  return (
    <>
      <button
        type="button"
        className={button}
        disabled={busy}
        onClick={() => {
          setTargets(rows.filter((row) => selectedIds.has(row.generation.id)));
          setIncludeRecipe(false);
          setOpen(true);
        }}
      >
        {t("export.action", { defaultValue: "Export originals" })}
      </button>
      {open && (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center bg-black/45 p-4"
          onClick={close}
        >
          <div
            ref={ref}
            tabIndex={-1}
            role="dialog"
            aria-modal="true"
            aria-label={t("export.action", {
              defaultValue: "Export originals",
            })}
            onClick={(event) => event.stopPropagation()}
            className="studio-floating-panel max-h-[calc(100dvh-2rem)] w-full max-w-md overflow-y-auto rounded-xl p-5 outline-none"
          >
            <h2 className="text-base font-semibold text-foreground">
              {t("export.action", { defaultValue: "Export originals" })}
            </h2>
            <p className="mt-3 text-sm text-foreground">
              {t("export.scope", {
                records: targets.length,
                images: targets.reduce(
                  (count, row) => count + row.images.length,
                  0,
                ),
                defaultValue:
                  "{{records}} generation record(s), {{images}} image(s)",
              })}
            </p>
            <p className="mt-2 text-xs leading-relaxed text-muted">
              {t("export.description", {
                defaultValue:
                  "Every image in the selected records is copied in its original format. Existing files are kept; matching names receive a numeric suffix.",
              })}
            </p>
            <label className="mt-4 flex items-start gap-2 text-sm text-foreground">
              <input
                type="checkbox"
                checked={includeRecipe}
                onChange={(event) => setIncludeRecipe(event.target.checked)}
                className="focus-ring mt-0.5 h-4 w-4 accent-primary"
              />
              {t("export.recipe", {
                defaultValue: "Include a JSON recipe beside each image",
              })}
            </label>
            <p className="mt-2 text-xs text-muted">
              {t("export.privacy", {
                defaultValue:
                  "Recipes include prompts and generation settings. Credentials, endpoints and local file paths are omitted.",
              })}
            </p>
            <div className="mt-5 flex flex-wrap justify-end gap-2">
              <button type="button" className={button} onClick={close}>
                {t("export.cancel", { defaultValue: "Cancel" })}
              </button>
              <button
                type="button"
                disabled={busy || !targets.length}
                className={button}
                onClick={() => {
                  close();
                  void start(
                    targets.map((row) => row.generation.id),
                    includeRecipe,
                  );
                }}
              >
                {t("export.chooseFolder", {
                  defaultValue: "Choose export folder",
                })}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
