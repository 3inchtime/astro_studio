import { Filter, RotateCcw, Search } from "lucide-react";
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
      <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-[15px] font-semibold tracking-tight text-foreground">
            {config.title}
          </h2>
          {total > 0 && (
            <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-[10px] font-medium tabular-nums text-muted">
              {total}
            </span>
          )}
        </div>

        <div className="flex w-full min-w-0 flex-col gap-2 sm:flex-row sm:items-center xl:w-auto">
          <label className="relative min-w-0 flex-1 xl:w-80 xl:flex-none">
            <Search
              size={13}
              className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted/60"
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
          <div className="flex items-center gap-2">
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

      <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {config.fields.map((field) => {
          if (field.type === "date") {
            return (
              <label key={field.key} className="flex min-w-0 flex-col gap-1">
                <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60">
                  {field.label}
                </span>
                <input
                  type="date"
                  value={field.value}
                  onChange={(event) => field.onChange(event.target.value)}
                  className="h-[34px] rounded-[10px] border border-border-subtle bg-subtle/35 px-3 text-[12px] text-foreground outline-none transition-colors focus:border-border focus:bg-surface"
                />
                </label>
            );
          }

          return (
            <label
              key={field.key}
              className="flex min-w-0 flex-col gap-1"
            >
              <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60">
                {field.label}
              </span>
              <select
                value={field.value}
                onChange={(event) => field.onChange(event.target.value)}
                className="select-control h-[34px] min-w-0 rounded-[10px] border border-border-subtle bg-subtle/35 px-3 text-[12px] text-foreground outline-none transition-colors focus:border-border focus:bg-surface"
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
      </div>
    </div>
  );
}
