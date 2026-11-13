import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { deleteGeneration } from "../../lib/api";
import ConfirmDialog from "../common/ConfirmDialog";

export default function GenerationDeleteDialog({ generationId, onClose, onDeleted }: { generationId: string; onClose: () => void; onDeleted: (id: string) => void }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const busyRef = useRef(false);
  async function confirm() {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setError(null);
    try { await deleteGeneration(generationId); onDeleted(generationId); onClose(); }
    catch { setError(t("studio.collection.actionError", { defaultValue: "The change could not be saved. Please try again." })); }
    finally { busyRef.current = false; setBusy(false); }
  }
  return <ConfirmDialog open title={t("studio.collection.deleteRecord", { defaultValue: "Move this generation to trash?" })} description={t("studio.collection.deleteRecordImages", { defaultValue: "All images in this generation record will move to trash." })} confirmLabel={t("studio.collection.bulkDelete", { defaultValue: "Move to trash" })} cancelLabel={t("favorites.cancel")} onConfirm={() => void confirm()} onCancel={onClose} loading={busy} error={error} />;
}
