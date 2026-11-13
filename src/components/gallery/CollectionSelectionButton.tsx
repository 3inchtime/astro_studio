import { useTranslation } from "react-i18next";

interface Props {
  active: boolean;
  disabled: boolean;
  onClick: () => void;
}

export default function CollectionSelectionButton({ active, disabled, onClick }: Props) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-pressed={active}
      disabled={disabled}
      onClick={onClick}
      className="studio-control-subtle focus-ring h-9 shrink-0 rounded-lg px-3 text-xs font-medium disabled:opacity-40"
    >
      {active
        ? t("studio.collection.doneSelecting", { defaultValue: "Done" })
        : t("studio.collection.select", { defaultValue: "Select" })}
    </button>
  );
}
