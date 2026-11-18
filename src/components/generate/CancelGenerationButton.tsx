import { useState } from "react";
import { useTranslation } from "react-i18next";
import { cancelGenerationJob } from "../../lib/api";

export default function CancelGenerationButton({ jobId }: { jobId: string }) {
  const { t } = useTranslation();
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);

  async function cancel() {
    setPending(true);
    setFailed(false);
    try {
      await cancelGenerationJob(jobId);
      // The committed terminal event or the next history read updates the bubble.
    } catch {
      setPending(false);
      setFailed(true);
    }
  }

  return <>
    <button type="button" className="studio-control focus-ring mt-2 rounded-lg px-3 py-1 text-xs"
      disabled={pending} onClick={() => void cancel()}>{t("generate.cancelTask")}</button>
    {failed && <p role="alert" className="mt-1 text-xs text-error">{t("generate.cancelTaskFailed")}</p>}
  </>;
}
