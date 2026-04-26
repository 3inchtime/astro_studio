import { useTranslation } from "react-i18next";

export default function GenerationLoadingScene() {
  const { t } = useTranslation();

  return (
    <div className="generation-loading-shell" role="status" aria-live="polite">
      <div className="generation-loading-scene" aria-hidden="true">
        <div className="generation-loading-orb">
          <div className="generation-loading-orb-halo" />
          <div className="generation-loading-orb-ring" />
          <div className="generation-loading-orb-core" />
          <span className="generation-loading-spark generation-loading-spark-a" />
          <span className="generation-loading-spark generation-loading-spark-b" />
          <span className="generation-loading-spark generation-loading-spark-c" />
        </div>
      </div>

      <div className="generation-loading-copy">
        <p className="generation-loading-title">{t("generate.loading.title")}</p>
        <p className="generation-loading-subtitle">{t("generate.loading.subtitle")}</p>

        <div className="generation-loading-meta" aria-hidden="true">
          <div className="generation-loading-dots">
            <span />
            <span />
            <span />
          </div>
          <div className="generation-loading-progress">
            <span />
          </div>
        </div>
      </div>
    </div>
  );
}
