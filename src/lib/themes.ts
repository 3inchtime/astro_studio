export type ThemeAppearance = "light" | "dark";

export type ThemeId =
  | "pure-light"
  | "pure-black"
  | "ocean-depths"
  | "sunset-boulevard"
  | "forest-canopy"
  | "modern-minimalist"
  | "golden-hour"
  | "arctic-frost"
  | "desert-rose"
  | "tech-innovation"
  | "botanical-garden"
  | "midnight-galaxy";

export interface ThemeDefinition {
  id: ThemeId;
  name: string;
  description: string;
  nameKey: string;
  descriptionKey: string;
  appearance: ThemeAppearance;
  swatches: [string, string, string, string];
  variables: Record<string, string>;
}

const LIGHT_THEME_BASE = {
  "--color-primary-foreground": "#FFFFFF",
  "--color-success": "#2E9E6E",
  "--color-warning": "#D4912A",
  "--color-error": "#D14545",
  "--shadow-card": "0 1px 2px rgba(45, 42, 38, 0.03), 0 4px 16px rgba(45, 42, 38, 0.04)",
  "--shadow-float": "0 2px 4px rgba(45, 42, 38, 0.04), 0 12px 32px rgba(45, 42, 38, 0.06)",
  "--shadow-panel": "0 -1px 4px rgba(45, 42, 38, 0.02), 0 -4px 20px rgba(45, 42, 38, 0.04)",
  "--shadow-button": "0 1px 3px rgba(45, 42, 38, 0.06)",
  "--shadow-inset": "inset 0 1px 2px rgba(45, 42, 38, 0.04)",
};

const DARK_THEME_BASE = {
  "--color-primary-foreground": "#FFFFFF",
  "--color-success": "#49C08A",
  "--color-warning": "#E2A94B",
  "--color-error": "#F06A6A",
  "--shadow-card": "0 1px 2px rgba(0, 0, 0, 0.16), 0 4px 16px rgba(0, 0, 0, 0.12)",
  "--shadow-float": "0 2px 4px rgba(0, 0, 0, 0.2), 0 12px 32px rgba(0, 0, 0, 0.16)",
  "--shadow-panel": "0 -1px 4px rgba(0, 0, 0, 0.16), 0 -4px 20px rgba(0, 0, 0, 0.12)",
  "--shadow-button": "0 1px 3px rgba(0, 0, 0, 0.18)",
  "--shadow-inset": "inset 0 1px 2px rgba(0, 0, 0, 0.2)",
};

function createLightTheme(
  id: ThemeId,
  name: string,
  description: string,
  swatches: [string, string, string, string],
  overrides: Partial<Record<string, string>>,
): ThemeDefinition {
  return {
    id,
    name,
    description,
    nameKey: `themes.${id}.name`,
    descriptionKey: `themes.${id}.description`,
    appearance: "light",
    swatches,
    variables: {
      ...LIGHT_THEME_BASE,
      "--color-primary": swatches[0],
      "--color-accent": swatches[1],
      "--color-lavender": swatches[2],
      "--color-lavender-light": overrides["--color-lavender-light"] ?? `${swatches[2]}22`,
      "--color-background": "#F7F4EF",
      "--color-surface": "#FFFFFF",
      "--color-surface-elevated": "#FFFFFF",
      "--color-border": "#E5DDD2",
      "--color-border-subtle": "#EEE6DA",
      "--color-foreground": "#2F2A26",
      "--color-muted": "#847A73",
      "--color-subtle": "#F1ECE4",
      "--color-bubble": `${swatches[2]}26`,
      "--color-info": swatches[0],
      "--color-glass": "rgba(255, 255, 252, 0.78)",
      "--color-glass-strong": "rgba(255, 255, 252, 0.92)",
      "--color-glass-dark": "rgba(255, 255, 252, 0.52)",
      ...overrides,
    },
  };
}

function createDarkTheme(
  id: ThemeId,
  name: string,
  description: string,
  swatches: [string, string, string, string],
  overrides: Partial<Record<string, string>>,
): ThemeDefinition {
  return {
    id,
    name,
    description,
    nameKey: `themes.${id}.name`,
    descriptionKey: `themes.${id}.description`,
    appearance: "dark",
    swatches,
    variables: {
      ...DARK_THEME_BASE,
      "--color-primary": swatches[1],
      "--color-accent": swatches[2],
      "--color-lavender": swatches[2],
      "--color-lavender-light": overrides["--color-lavender-light"] ?? `${swatches[2]}22`,
      "--color-background": "#17161A",
      "--color-surface": "#201E24",
      "--color-surface-elevated": "#27242D",
      "--color-border": "#35313C",
      "--color-border-subtle": "#2A2630",
      "--color-foreground": "#F2EFE9",
      "--color-muted": "#A59D95",
      "--color-subtle": "#26232C",
      "--color-bubble": `${swatches[1]}1E`,
      "--color-info": swatches[1],
      "--color-glass": "rgba(30, 27, 34, 0.78)",
      "--color-glass-strong": "rgba(30, 27, 34, 0.92)",
      "--color-glass-dark": "rgba(30, 27, 34, 0.52)",
      ...overrides,
    },
  };
}

export const THEME_CATALOG: ThemeDefinition[] = [
  createLightTheme(
    "pure-light",
    "Pure Light",
    "A bright white workspace with restrained grayscale contrast.",
    ["#111111", "#6B7280", "#E5E7EB", "#FFFFFF"],
    {
      "--color-background": "#FFFFFF",
      "--color-surface": "#FFFFFF",
      "--color-surface-elevated": "#FFFFFF",
      "--color-border": "#E5E7EB",
      "--color-border-subtle": "#F1F5F9",
      "--color-foreground": "#111111",
      "--color-muted": "#6B7280",
      "--color-subtle": "#F8FAFC",
      "--color-bubble": "rgba(17, 17, 17, 0.06)",
      "--color-primary": "#111111",
      "--color-accent": "#6B7280",
      "--color-lavender": "#E5E7EB",
      "--color-lavender-light": "rgba(229, 231, 235, 0.5)",
      "--color-info": "#111111",
      "--color-glass": "rgba(255, 255, 255, 0.86)",
      "--color-glass-strong": "rgba(255, 255, 255, 0.96)",
      "--color-glass-dark": "rgba(255, 255, 255, 0.72)",
      "--shadow-card": "0 1px 2px rgba(15, 23, 42, 0.04), 0 8px 24px rgba(15, 23, 42, 0.05)",
      "--shadow-float": "0 4px 12px rgba(15, 23, 42, 0.06), 0 18px 42px rgba(15, 23, 42, 0.08)",
    },
  ),
  createDarkTheme(
    "pure-black",
    "Pure Black",
    "A near-monochrome black workspace with sharp high-contrast edges.",
    ["#000000", "#FFFFFF", "#8B8B8B", "#F5F5F5"],
    {
      "--color-background": "#000000",
      "--color-surface": "#0A0A0A",
      "--color-surface-elevated": "#111111",
      "--color-border": "#1F1F1F",
      "--color-border-subtle": "#141414",
      "--color-foreground": "#FAFAFA",
      "--color-muted": "#A3A3A3",
      "--color-subtle": "#121212",
      "--color-bubble": "rgba(255, 255, 255, 0.08)",
      "--color-primary": "#FFFFFF",
      "--color-accent": "#8B8B8B",
      "--color-lavender": "#D4D4D4",
      "--color-lavender-light": "rgba(212, 212, 212, 0.12)",
      "--color-info": "#FFFFFF",
      "--color-glass": "rgba(8, 8, 8, 0.82)",
      "--color-glass-strong": "rgba(8, 8, 8, 0.94)",
      "--color-glass-dark": "rgba(8, 8, 8, 0.64)",
      "--shadow-card": "0 1px 2px rgba(255, 255, 255, 0.03), 0 8px 24px rgba(0, 0, 0, 0.5)",
      "--shadow-float": "0 4px 12px rgba(255, 255, 255, 0.03), 0 18px 42px rgba(0, 0, 0, 0.65)",
    },
  ),
  createDarkTheme(
    "ocean-depths",
    "Ocean Depths",
    "Professional maritime blues with calm, high-trust contrast.",
    ["#1A2332", "#2D8B8B", "#A8DADC", "#F1FAEE"],
    {
      "--color-background": "#111A23",
      "--color-surface": "#18232E",
      "--color-surface-elevated": "#213140",
      "--color-border": "#274454",
      "--color-border-subtle": "#1E3442",
      "--color-foreground": "#EDF7F4",
      "--color-muted": "#8FB4B3",
      "--color-subtle": "#16232E",
      "--color-bubble": "rgba(45, 139, 139, 0.18)",
    },
  ),
  createLightTheme(
    "sunset-boulevard",
    "Sunset Boulevard",
    "Warm creative oranges with cinematic dusk contrast.",
    ["#E76F51", "#F4A261", "#E9C46A", "#264653"],
    {
      "--color-background": "#FBF4EA",
      "--color-surface": "#FFF9F2",
      "--color-surface-elevated": "#FFFFFF",
      "--color-border": "#EFD4BE",
      "--color-border-subtle": "#F7E4D4",
      "--color-foreground": "#264653",
      "--color-muted": "#8D6F5B",
      "--color-subtle": "#F8EADF",
      "--color-bubble": "rgba(244, 162, 97, 0.18)",
      "--color-info": "#E76F51",
    },
  ),
  createDarkTheme(
    "forest-canopy",
    "Forest Canopy",
    "Grounded botanical depth with saturated forest greens.",
    ["#2D4A2B", "#7D8471", "#A4AC86", "#FAF9F6"],
    {
      "--color-background": "#141A14",
      "--color-surface": "#1B241B",
      "--color-surface-elevated": "#223022",
      "--color-border": "#344433",
      "--color-border-subtle": "#2A3729",
      "--color-foreground": "#F5F3EC",
      "--color-muted": "#A3AD94",
      "--color-subtle": "#202920",
      "--color-bubble": "rgba(125, 132, 113, 0.18)",
      "--color-primary": "#A4AC86",
      "--color-accent": "#7D8471",
      "--color-info": "#A4AC86",
    },
  ),
  createLightTheme(
    "modern-minimalist",
    "Modern Minimalist",
    "Quiet grayscale for a clean studio workspace.",
    ["#36454F", "#708090", "#D3D3D3", "#FFFFFF"],
    {
      "--color-background": "#F5F6F7",
      "--color-surface": "#FFFFFF",
      "--color-surface-elevated": "#FFFFFF",
      "--color-border": "#D9DEE2",
      "--color-border-subtle": "#E7EBEE",
      "--color-foreground": "#36454F",
      "--color-muted": "#708090",
      "--color-subtle": "#EEF1F3",
      "--color-bubble": "rgba(112, 128, 144, 0.14)",
      "--color-primary": "#36454F",
      "--color-info": "#36454F",
    },
  ),
  createLightTheme(
    "golden-hour",
    "Golden Hour",
    "Editorial gold and terracotta with soft warmth.",
    ["#F4A900", "#C1666B", "#D4B896", "#4A403A"],
    {
      "--color-background": "#FBF3E7",
      "--color-surface": "#FFF9F2",
      "--color-surface-elevated": "#FFFFFF",
      "--color-border": "#E9D2B7",
      "--color-border-subtle": "#F3E3CF",
      "--color-foreground": "#4A403A",
      "--color-muted": "#8E7563",
      "--color-subtle": "#F7EADD",
      "--color-bubble": "rgba(193, 102, 107, 0.16)",
      "--color-info": "#C1666B",
    },
  ),
  createLightTheme(
    "arctic-frost",
    "Arctic Frost",
    "Cool steel blues with crisp clinical clarity.",
    ["#D4E4F7", "#4A6FA5", "#C0C0C0", "#FAFAFA"],
    {
      "--color-background": "#F3F8FC",
      "--color-surface": "#FCFEFF",
      "--color-surface-elevated": "#FFFFFF",
      "--color-border": "#D5E1EC",
      "--color-border-subtle": "#E6EEF4",
      "--color-foreground": "#2B4668",
      "--color-muted": "#6A83A3",
      "--color-subtle": "#EAF1F7",
      "--color-bubble": "rgba(74, 111, 165, 0.14)",
      "--color-info": "#4A6FA5",
    },
  ),
  createLightTheme(
    "desert-rose",
    "Desert Rose",
    "Dusty rose neutrals with boutique softness.",
    ["#D4A5A5", "#B87D6D", "#E8D5C4", "#5D2E46"],
    {
      "--color-background": "#FBF3EF",
      "--color-surface": "#FFF9F6",
      "--color-surface-elevated": "#FFFFFF",
      "--color-border": "#E8D3CB",
      "--color-border-subtle": "#F3E4DE",
      "--color-foreground": "#5D2E46",
      "--color-muted": "#9B6D6D",
      "--color-subtle": "#F7EBE6",
      "--color-bubble": "rgba(212, 165, 165, 0.18)",
      "--color-info": "#B87D6D",
    },
  ),
  createDarkTheme(
    "tech-innovation",
    "Tech Innovation",
    "Electric startup energy with neon cyan contrast.",
    ["#0066FF", "#00FFFF", "#1E1E1E", "#FFFFFF"],
    {
      "--color-background": "#0D1320",
      "--color-surface": "#121B2E",
      "--color-surface-elevated": "#17253F",
      "--color-border": "#213A64",
      "--color-border-subtle": "#182A47",
      "--color-foreground": "#F7FBFF",
      "--color-muted": "#8EB9D8",
      "--color-subtle": "#101C30",
      "--color-bubble": "rgba(0, 255, 255, 0.14)",
      "--color-primary": "#0066FF",
      "--color-accent": "#00FFFF",
      "--color-lavender": "#80EAFF",
      "--color-lavender-light": "rgba(0, 255, 255, 0.16)",
      "--color-info": "#00FFFF",
    },
  ),
  createLightTheme(
    "botanical-garden",
    "Botanical Garden",
    "Fresh natural greens with vibrant organic accents.",
    ["#4A7C59", "#F9A620", "#B7472A", "#F5F3ED"],
    {
      "--color-background": "#F6F4EC",
      "--color-surface": "#FDFBF6",
      "--color-surface-elevated": "#FFFFFF",
      "--color-border": "#D9DFC8",
      "--color-border-subtle": "#EBE9DA",
      "--color-foreground": "#2E4B34",
      "--color-muted": "#6C7F67",
      "--color-subtle": "#EEF1E4",
      "--color-bubble": "rgba(74, 124, 89, 0.14)",
      "--color-info": "#4A7C59",
    },
  ),
  createDarkTheme(
    "midnight-galaxy",
    "Midnight Galaxy",
    "Cosmic deep-space purples with luminous lavender.",
    ["#2B1E3E", "#4A4E8F", "#A490C2", "#E6E6FA"],
    {
      "--color-background": "#130F1C",
      "--color-surface": "#1A1527",
      "--color-surface-elevated": "#231C35",
      "--color-border": "#362A52",
      "--color-border-subtle": "#281F3E",
      "--color-foreground": "#F4F1FF",
      "--color-muted": "#B2A8D2",
      "--color-subtle": "#1E1830",
      "--color-bubble": "rgba(164, 144, 194, 0.18)",
      "--color-info": "#A490C2",
    },
  ),
];

export const DEFAULT_THEME_ID: ThemeId = "pure-light";

const THEME_MAP = new Map<ThemeId, ThemeDefinition>(
  THEME_CATALOG.map((theme) => [theme.id, theme]),
);

export function isThemeId(value: string | null | undefined): value is ThemeId {
  return Boolean(value) && THEME_MAP.has(value as ThemeId);
}

export function getThemeCatalogEntry(themeId: ThemeId): ThemeDefinition {
  return THEME_MAP.get(themeId) ?? THEME_MAP.get(DEFAULT_THEME_ID)!;
}

function resolveTranslatedThemeText(
  key: string,
  fallback: string,
  translate: (key: string) => string,
): string {
  const translated = translate(key);
  return translated === key ? fallback : translated;
}

export function getThemeName(
  theme: ThemeDefinition,
  translate: (key: string) => string,
): string {
  return resolveTranslatedThemeText(theme.nameKey, theme.name, translate);
}

export function getThemeDescription(
  theme: ThemeDefinition,
  translate: (key: string) => string,
): string {
  return resolveTranslatedThemeText(
    theme.descriptionKey,
    theme.description,
    translate,
  );
}

export function resolveThemeId(value: string | null | undefined): ThemeId {
  if (value === "dark") {
    return "pure-black";
  }

  if (value === "light") {
    return DEFAULT_THEME_ID;
  }

  if (isThemeId(value)) {
    return value;
  }

  return DEFAULT_THEME_ID;
}
