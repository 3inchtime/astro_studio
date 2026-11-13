import { ChevronDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  IMAGE_MODEL_CATALOG,
  getImageModelCatalogEntry,
  isGptImage26,
} from "../../lib/modelCatalog";
import { image26ValidationError } from "../../lib/image26Options";
import type {
  ImageBackground,
  ImageInputFidelity,
  ImageModel,
  ImageModeration,
  ImageOutputFormat,
  ImageQuality,
  ImageSize,
} from "../../types";
import type { GenerationComposerProps } from "./GenerationComposer";
import ImageAdvancedControls from "./ImageAdvancedControls";

const outputFormatOptions: ImageOutputFormat[] = [
  "png",
  "jpeg",
  "webp",
  "svg",
  "gif",
];
const backgroundOptions: ImageBackground[] = ["auto", "opaque", "transparent"];
const moderationOptions: ImageModeration[] = ["auto", "low"];
const inputFidelityOptions: ImageInputFidelity[] = ["high", "low"];
const imageCountOptions = Array.from({ length: 10 }, (_, index) => index + 1);

type Props = Pick<
  GenerationComposerProps,
  | "advancedOptions"
  | "onAdvancedOptionsChange"
  | "outputCompression"
  | "onOutputCompressionChange"
  | "imageModel"
  | "size"
  | "aspectRatio"
  | "resolution"
  | "onSizeChange"
  | "onAspectRatioChange"
  | "onResolutionChange"
  | "quality"
  | "background"
  | "outputFormat"
  | "moderation"
  | "inputFidelity"
  | "imageCount"
  | "editSources"
  | "onModelChange"
  | "onQualityChange"
  | "onBackgroundChange"
  | "onOutputFormatChange"
  | "onModerationChange"
  | "onInputFidelityChange"
  | "onImageCountChange"
>;

export default function GenerationParameters({
  advancedOptions = {},
  onAdvancedOptionsChange,
  outputCompression,
  onOutputCompressionChange,
  imageModel,
  size = "auto",
  aspectRatio,
  resolution,
  onSizeChange,
  onAspectRatioChange,
  onResolutionChange,
  quality,
  background,
  outputFormat,
  moderation,
  inputFidelity,
  imageCount,
  editSources,
  onModelChange,
  onQualityChange,
  onBackgroundChange,
  onOutputFormatChange,
  onModerationChange,
  onInputFidelityChange,
  onImageCountChange,
}: Props) {
  const { t } = useTranslation();
  const hasEditSources = editSources.length > 0;
  // ── Model catalog ───────────────────────────────────────────────────────────
  const modelCatalogEntry = getImageModelCatalogEntry(imageModel);
  const { parameterCapabilities } = modelCatalogEntry;
  const validationError = image26ValidationError({
    model: imageModel,
    format: outputFormat,
    size,
    count: imageCount,
    background,
    editing: hasEditSources,
    options: advancedOptions,
    compression: outputCompression,
  });
  const showQuality = parameterCapabilities.qualities.length > 1;
  const showBackground = parameterCapabilities.backgrounds.length > 1;
  const showImageCount = parameterCapabilities.imageCounts.length > 0;
  const showOutputFormat = parameterCapabilities.outputFormats.length > 1;
  const showModeration = parameterCapabilities.moderationLevels.length > 1;
  const showSourceEditing = modelCatalogEntry.supportsEdit;
  const showInputFidelity =
    showSourceEditing &&
    editSources.length > 0 &&
    parameterCapabilities.inputFidelityOptions.length > 1;
  const hasAdvancedSettings =
    showQuality ||
    showBackground ||
    showOutputFormat ||
    showModeration ||
    showInputFidelity ||
    isGptImage26(imageModel);
  const providerLabel =
    (
      {
        google: "Google",
        openai: "OpenAI",
        xai: "xAI",
        volcengine: "Volcengine",
      } as Record<string, string>
    )[modelCatalogEntry.provider] ?? modelCatalogEntry.provider;
  return (
    <div className="min-w-0 bg-surface p-4">
      <div
        role="toolbar"
        data-variant="parameter-rail"
        aria-label={t("generate.parametersLabel")}
        className="mx-auto w-full"
      >
        <div className="mb-5 flex items-center justify-between gap-3 border-b border-border-subtle pb-4 text-xs text-muted">
          <span>
            {t("studio.imageProvider", { defaultValue: "Image provider" })}:{" "}
            <span className="font-medium text-foreground">{providerLabel}</span>
          </span>
        </div>
        <div
          data-testid="generation-parameter-row"
          className="grid w-full min-w-0 grid-cols-2 items-end gap-4"
        >
          <div className="col-span-2">
            <SelectField
              label={t("generate.modelLabel")}
              value={imageModel}
              onChange={(value) => onModelChange(value as ImageModel)}
              options={IMAGE_MODEL_CATALOG.map((entry) => ({
                value: entry.id,
                label: t(entry.i18nKey),
              }))}
            />
          </div>
          <h3 className="col-span-2 mt-2 border-t border-border-subtle pt-4 text-xs font-semibold text-foreground">
            {t("studio.parameters.output", { defaultValue: "Output" })}
          </h3>
          {parameterCapabilities.sizes.length > 1 &&
            !parameterCapabilities.aspectRatios && (
              <SelectField
                label={t("generate.sizeLabel")}
                value={
                  parameterCapabilities.sizes.includes(size) ? size : "custom"
                }
                onChange={(value) =>
                  value !== "custom" && onSizeChange?.(value as ImageSize)
                }
                options={[
                  ...parameterCapabilities.sizes.map((value) => ({
                    value,
                    label: value,
                  })),
                  ...(!parameterCapabilities.sizes.includes(size)
                    ? [{ value: "custom", label: size }]
                    : []),
                ]}
              />
            )}
          {parameterCapabilities.aspectRatios && (
            <SelectField
              label={t("generate.aspectRatioLabel")}
              value={aspectRatio ?? "1:1"}
              onChange={(value) => onAspectRatioChange?.(value)}
              options={parameterCapabilities.aspectRatios.map((value) => ({
                value,
                label: value,
              }))}
            />
          )}
          {parameterCapabilities.resolutions && (
            <SelectField
              label={t("generate.resolutionLabel")}
              value={resolution ?? parameterCapabilities.resolutions[0]}
              onChange={(value) => onResolutionChange?.(value)}
              options={parameterCapabilities.resolutions.map((value) => ({
                value,
                label: value,
              }))}
            />
          )}
          {showImageCount && (
            <SelectField
              label={t("generate.countLabel")}
              value={String(imageCount)}
              onChange={(value) => onImageCountChange(Number(value))}
              options={imageCountOptions
                .filter((value) =>
                  parameterCapabilities.imageCounts.includes(value),
                )
                .map((value) => ({
                  value: String(value),
                  label: t("generate.countValue", { count: value }),
                  disabled: outputFormat === "svg" && value !== 1,
                }))}
            />
          )}
        </div>
        {hasAdvancedSettings && (
          <details className="group mt-5 border-t border-border-subtle pt-2">
            <summary className="focus-ring flex min-h-9 cursor-pointer list-none items-center justify-between rounded-lg text-xs font-medium text-foreground [&::-webkit-details-marker]:hidden">
              <span>
                {t("studio.advancedSettings", {
                  defaultValue: "Advanced settings",
                })}
              </span>
              <ChevronDown
                size={14}
                className="text-muted transition-transform group-open:rotate-180"
              />
            </summary>
            <div className="mt-3 grid grid-cols-1 gap-3">
              {showQuality && (
                <SelectField
                  label={t("generate.qualityLabel")}
                  value={quality}
                  onChange={(value) => onQualityChange(value as ImageQuality)}
                  options={parameterCapabilities.qualities.map((value) => ({
                    value,
                    label: t(`generate.quality.${value}`),
                  }))}
                />
              )}
              {showBackground && (
                <SelectField
                  label={t("generate.backgroundLabel")}
                  value={background}
                  onChange={(value) =>
                    onBackgroundChange(value as ImageBackground)
                  }
                  options={backgroundOptions
                    .filter((value) =>
                      parameterCapabilities.backgrounds.includes(value),
                    )
                    .map((value) => ({
                      value,
                      label: t(`generate.background.${value}`),
                      disabled:
                        !["png", "webp", "svg"].includes(outputFormat) &&
                        value === "transparent",
                    }))}
                />
              )}
              {showOutputFormat && (
                <SelectField
                  label={t("generate.formatLabel")}
                  value={outputFormat}
                  onChange={(value) =>
                    onOutputFormatChange(value as ImageOutputFormat)
                  }
                  options={outputFormatOptions
                    .filter(
                      (value) =>
                        parameterCapabilities.outputFormats.includes(value) &&
                        !(hasEditSources && value === "svg"),
                    )
                    .map((value) => ({
                      value,
                      label: t(`generate.format.${value}`),
                      disabled:
                        background === "transparent" &&
                        !["png", "webp", "svg"].includes(value),
                    }))}
                />
              )}
              {showModeration && (
                <SelectField
                  label={t("generate.moderationLabel")}
                  value={moderation}
                  onChange={(value) =>
                    onModerationChange(value as ImageModeration)
                  }
                  options={moderationOptions
                    .filter((value) =>
                      parameterCapabilities.moderationLevels.includes(value),
                    )
                    .map((value) => ({
                      value,
                      label: t(`generate.moderation.${value}`),
                    }))}
                />
              )}
              {showInputFidelity && (
                <SelectField
                  label={t("generate.inputFidelityLabel")}
                  value={inputFidelity}
                  onChange={(value) =>
                    onInputFidelityChange(value as ImageInputFidelity)
                  }
                  options={inputFidelityOptions
                    .filter((value) =>
                      parameterCapabilities.inputFidelityOptions.includes(
                        value,
                      ),
                    )
                    .map((value) => ({
                      value,
                      label: t(`generate.inputFidelity.${value}`),
                    }))}
                />
              )}
            </div>
            {isGptImage26(imageModel) && onAdvancedOptionsChange && (
              <ImageAdvancedControls
                embedded
                value={advancedOptions}
                onChange={onAdvancedOptionsChange}
                format={outputFormat}
                editing={hasEditSources}
                size={size}
                onSizeChange={onSizeChange}
                compression={outputCompression}
                onCompressionChange={onOutputCompressionChange}
              />
            )}
          </details>
        )}
        {validationError && (
          <p role="alert" className="px-2 pt-2 text-xs text-error">
            {t(`generate.advanced.${validationError}`)}
          </p>
        )}
      </div>
    </div>
  );
}

interface SelectFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: Array<{ value: string; label: string; disabled?: boolean }>;
}

function SelectField({ label, value, onChange, options }: SelectFieldProps) {
  return (
    <label className="flex min-w-0 flex-col gap-1.5 text-xs text-muted">
      <span className="font-medium" title={label}>
        {label}
      </span>
      <select
        value={value}
        disabled={options.length === 1}
        onChange={(e) => onChange(e.target.value)}
        className="studio-input focus-ring select-control h-9 w-full min-w-0 cursor-pointer truncate rounded-lg px-3 pr-8 text-sm text-foreground"
      >
        {options.map((option) => (
          <option
            key={option.value}
            value={option.value}
            disabled={option.disabled}
          >
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}
