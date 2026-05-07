import { CalendarDays, Filter, RotateCcw, Search, SlidersHorizontal } from "lucide-react";
import type { GallerySearchConfig } from "../../lib/galleryFilterConfig";

interface GallerySearchBarProps {
  config: GallerySearchConfig;
  total: number;
  query: string;
  hasActiveFilters: boolean;
  onQueryChange: (query: string) => void;
  onSearch: () => void;
  onReset: () => void;
}

export default function GallerySearchBar({
  config,
  total,
  query,
  hasActiveFilters,
  onQueryChange,
  onSearch,
  onReset,
}: GallerySearchBarProps) {
  return (
    <div className="border-b border-border-subtle px-6 py-4">
      <div className="flex flex-col gap-3 xl:flex-row xl:items-end xl:justify-between">
        <div className="flex shrink-0 items-center gap-3 pb-0.5">
          <h2 className="text-[15px] font-semibold tracking-tight text-foreground">
            {config.title}
          </h2>
          {total > 0 && (
            <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-[10px] font-medium tabular-nums text-muted">
              {total}
            </span>
          )}
        </div>

        <div
          role="search"
          aria-label={`${config.title} filters`}
          className="flex w-full min-w-0 flex-wrap items-end gap-2 xl:flex-1 xl:justify-end"
        >
          <label className="relative min-w-[220px] flex-[1_1_260px]">
            <span className="mb-1 block text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60">
              {config.searchLabel}
            </span>
            <Search
              size={13}
              className="pointer-events-none absolute left-2.5 bottom-[10px] text-muted/60"
              strokeWidth={2}
            />
            <input
              value={query}
              onChange={(event) => onQueryChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  onSearch();
                }
              }}
              placeholder={config.searchPlaceholder}
              className="h-[34px] w-full rounded-[10px] border border-border-subtle bg-subtle/40 pl-7 pr-3 text-[12px] text-foreground placeholder:text-muted/50 transition-colors focus:border-border focus:bg-surface focus:outline-none"
            />
          </label>

          {config.fields.map((field) => {
            if (field.type === "date") {
              return (
                <label
                  key={field.key}
                  className="relative min-w-[140px] flex-[1_1_150px] xl:max-w-[170px]"
                >
                  <span className="mb-1 block text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60">
                    {field.label}
                  </span>
                  <CalendarDays
                    size={13}
                    className="pointer-events-none absolute left-2.5 bottom-[10px] text-muted/55"
                    strokeWidth={2}
                  />
                  <input
                    type="date"
                    value={field.value}
                    onChange={(event) => field.onChange(event.target.value)}
                    className="h-[34px] w-full rounded-[10px] border border-border-subtle bg-subtle/35 pl-7 pr-3 text-[12px] text-foreground outline-none transition-colors focus:border-border focus:bg-surface"
                  />
                </label>
              );
            }

            return (
              <label
                key={field.key}
                className="relative min-w-[170px] flex-[1_1_180px] xl:max-w-[220px]"
              >
                <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60">
                  {field.label}
                </span>
                <SlidersHorizontal
                  size={13}
                  className="pointer-events-none absolute left-2.5 bottom-[10px] text-muted/55"
                  strokeWidth={2}
                />
                <select
                  value={field.value}
                  onChange={(event) => field.onChange(event.target.value)}
                  className="select-control mt-1 h-[34px] w-full min-w-0 rounded-[10px] border border-border-subtle bg-subtle/35 pl-7 pr-8 text-[12px] text-foreground outline-none transition-colors focus:border-border focus:bg-surface"
                >
                  {field.options.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
            );
          })}

          <div className="flex flex-[0_0_auto] items-end gap-2">
            <button
              type="button"
              onClick={onSearch}
              className="inline-flex h-[34px] items-center justify-center gap-1.5 rounded-[10px] gradient-primary px-4 text-[12px] font-medium text-white shadow-button transition-transform hover:-translate-y-0.5"
            >
              <Filter size={13} />
              {config.applyFilters}
            </button>
            <button
              type="button"
              onClick={onReset}
              disabled={!hasActiveFilters}
              className="inline-flex h-[34px] items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle px-4 text-[12px] font-medium text-foreground/75 transition-colors hover:border-border hover:bg-subtle hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
            >
              <RotateCcw size={13} />
              {config.resetFilters}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
