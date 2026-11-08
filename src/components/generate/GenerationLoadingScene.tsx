import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";

export default function GenerationLoadingScene() {
  const { t } = useTranslation();

  return (
    <div
      className="flex min-h-36 w-full max-w-md items-start gap-3 rounded-xl border border-border-subtle bg-surface p-5"
      role="status"
      aria-live="polite"
    >
      <Loader2
        size={18}
        className="mt-0.5 shrink-0 animate-spin text-primary motion-reduce:animate-none"
        aria-hidden="true"
      />
      <div>
        <p className="text-[14px] font-medium text-foreground">
          {t("studio.generationWaiting", {
            defaultValue: "Waiting for image provider",
          })}
        </p>
        <p className="mt-1 text-[12px] leading-relaxed text-muted">
          {t("studio.generationWaitingHint", {
            defaultValue:
              "Your request is processing. Images will appear when ready.",
          })}
        </p>
      </div>
    </div>
  );
}
