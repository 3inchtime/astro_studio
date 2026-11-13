---
name: Astro Studio
description: A precise image-first desktop workbench for local AI image creation.
colors:
  primary: "#315eea"
  primary-foreground: "#FFFFFF"
  background: "#F5F6F8"
  canvas: "#F2F3F5"
  rail: "#EFF1F4"
  surface: "#FFFFFF"
  surface-elevated: "#FFFFFF"
  border: "#DCE0E6"
  border-subtle: "#E7E9EE"
  control-border: "#888b8f"
  foreground: "#111111"
  muted: "#676d7b"
  subtle: "#EFF1F4"
  success: "#247b56"
  warning: "#92641d"
  error: "#c24040"
typography:
  title:
    fontFamily: '"SF Pro Text", "Segoe UI Variable", system-ui, -apple-system, sans-serif'
    fontSize: "calc(20px * var(--app-font-scale))"
    fontWeight: 600
    letterSpacing: "-0.025em"
  body:
    fontFamily: '"SF Pro Text", "Segoe UI Variable", system-ui, -apple-system, sans-serif'
    fontSize: "max(12px, calc(14px * var(--app-font-scale)))"
    lineHeight: 1.5
  body-sm:
    fontSize: "max(12px, calc(13px * var(--app-font-scale)))"
  label:
    fontSize: "max(12px, calc(12px * var(--app-font-scale)))"
  mono:
    fontFamily: '"SF Mono", "Cascadia Code", ui-monospace, monospace'
rounded:
  button: "8px"
  input: "8px"
  card: "12px"
  window: "12px"
  thumbnail: "10px"
  badge: "6px"
spacing:
  unit: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.primary-foreground}"
    typography: "{typography.body}"
    rounded: "{rounded.button}"
    padding: "0 16px"
    height: "40px"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.button}"
    padding: "6px 12px"
    height: "36px"
  button-subtle:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.button}"
    padding: "6px 12px"
    height: "36px"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.input}"
    padding: "6px 10px"
    height: "36px"
  navigation-item:
    textColor: "{colors.muted}"
    rounded: "{rounded.button}"
    padding: "4px 2px"
    width: "64px"
    height: "48px"
  metadata-chip:
    backgroundColor: "{colors.subtle}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.badge}"
    padding: "2px 8px"
  image-card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.card}"
  canvas-tool:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.button}"
    height: "36px"
    width: "36px"
  favorite-action:
    textColor: "{colors.muted}"
    rounded: "{rounded.button}"
    height: "32px"
    width: "32px"
  button-danger:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.error}"
    rounded: "{rounded.button}"
---

# Design System: Astro Studio

## Overview

**Creative North Star: "Precise image-first desktop workbench"**

Operate mode for creation, editing, settings and organization; image-led viewing for previews. Product function and familiar desktop behavior take priority over decorative styling. The 2026-11-08 redesign, deepened on 2026-11-13, replaces layered glass, tiny low-contrast copy and crowded composer overlays with a coherent workbench.

The existing visual world is a precise image-first desktop workbench. Images occupy the flexible center; tools and metadata live in stable surrounding surfaces. Local platform typography, flat panels and short feedback keep repeated work legible across twelve themes and eight locales. Existing routes, IPC contracts, editor state and local data remain authoritative.

**Key Characteristics:**

- Neutral image inspection, theme-aware controls.
- Readable semantic typography with a 12px floor.
- Stable desktop docks with exclusive compact drawers.
- Flat content surfaces; lifted overlays and explicit focus.

## Colors

A neutral workspace frames the artwork; restrained primary and status colors identify actions and state.

The frontmatter records the **resolved default pure-light theme**, after the contrast corrections in `src/lib/themes.ts`. Runtime theme variables override bootstrap values in `src/styles/globals.css`; keep the theme catalog as the source of truth rather than copying the bootstrap palette. Other presets keep their established accents and the same semantic roles. The pure-black preset uses graphite surfaces, with a light primary treatment and contrast-selected action text.

### Primary

- **Workbench blue** (`primary`): the main action, active destination and focus treatment.
- **Action ink** (`primary-foreground`): selected by the theme factory for contrast against primary; do not assume it is white in every theme.
- **Success, warning and error**: corrected semantic status colors. Existing accent, lavender and bubble variables remain available to incumbent features; they do not create additional workbench hierarchies.

### Neutral

- **Workspace and stage** (`background`, `canvas`): application background and image evaluation surface.
- **Navigation and quiet fill** (`rail`, `subtle`): location and metadata grouping.
- **Opaque panels** (`surface`, `surface-elevated`): forms, inspectors and overlays.
- **Ink and secondary ink** (`foreground`, `muted`): functional text hierarchy.
- **Separators and control edges** (`border`, `border-subtle`, `control-border`): subtle structural division and stronger interactive affordances.

**The Neutral Stage Rule.** Image inspection and canvas surfaces use the neutral canvas token regardless of the selected accent theme.

**The Readable Control Rule.** Functional labels use solid semantic colors; theme generation corrects muted, primary and status colors against supported surfaces to at least 4.5:1, and dedicated control borders to at least 3:1.

Sidecar tonal ramps are non-normative swatch previews synthesized from the recorded colors; they are not additional application tokens.

## Typography

**Body and title font:** the local platform sans stack recorded in frontmatter. **Mono font:** the local platform mono stack. System fallback covers CJK text; there is no network font dependency and no separate marketing display face.

### Hierarchy

- **Title:** the shared page-heading role; the generation bar uses the existing 16px semantic base step and semibold weight within its 48px header.
- **Body:** the application shell's main reading size and line height.
- **Body small:** supporting descriptions and dense form content.
- **Label:** secondary metadata and navigation labels.
- **Mono:** code-like values where the existing component calls for it.

The font preference sets `--app-font-scale` to 0.92 / 1 / 1.1 for small / medium / large. Additional existing semantic steps cover 15px body-large and 16px base at the default scale. These are support roles rather than a new display hierarchy.

**The Semantic Type Rule.** Use the semantic size tokens and the shared font preference scale; secondary text never falls below the 12px floor.

## Layout

A four-pixel base rhythm supports compact repeated controls. The application has an 80px icon-and-label navigation rail, a collapsible contextual sidebar and a flexible main surface. Context width is keyboard-resizable and versioned in preferences (180–400px, default 260px). Navigation groups creation, projects and library; trash belongs to library and settings stays at the bottom. Existing URLs stay valid.

Conversation and canvas document docks appear at 1440px; generation parameters, canvas inspector and collection details dock at 1200px. Below each threshold they become drawers. The main surface must fit a 900×600 window. Conversation and generation parameter drawers share one active-drawer state; canvas documents and inspector are mutually exclusive.

- **Generation:** chronological image feed, bottom reference/prompt/action dock capped at 35% of workspace height and a 300px parameter inspector. Composer content scrolls above a persistent submit row. The composer retains a 152px minimum and its writing field a 64px minimum. Mode controls are neutral 32px segments; prompt metadata and contextual actions share a quiet row. Result images are centered and sized from the remaining feed viewport budget. The feed observes both viewport and content resize and follows new content only while the user is already following the latest messages. Model, size and count stay visible; advanced parameters use disclosure and container-width columns. A stable portal host preserves drafts and disclosures across breakpoints.
- **Collections:** one header/filter/results/detail pattern, docked detail above the inspector threshold and modal detail below it. Measured-width masonry explicitly distributes image records into columns with 16px gutters, filling three/four/five columns in the standard 900/1200/1440px collection views; opening a detail dock reduces the available grid width. Appending a page preserves prior column placement. Batch selection addresses visible loaded image-bearing generation records separately from the inspected record. Its action bar appears only after entering selection or selecting a record, and select-all excludes rows without images.
- **Canvas:** 220px document dock, flexible neutral stage and 300px inspector. Drawing tools form a vertical left rail; fit/view/history controls stay at the bottom. Properties/Layers/Generate tabs persist, default to Properties and do not switch automatically with selection. Borderless tools form quiet groups separated by subtle rules. Neutral paper and the idle frame stay quiet; blue dashed frame treatment appears while editing the frame. The labeled Fit action is explicit and saved viewport state is preserved.
- **Projects and settings:** covers, recent activity and direct continuation for projects; flat grouped forms with visible dirty, pending, success and failure states for settings. Settings show compact previews for all twelve themes, retaining full names/descriptions through title and accessible description; language and font controls appear early in the form. Selected settings choices use neutral surfaces and primary borders/ink to retain contrast. Projects place the context toggle inline with the page heading, without a duplicate toolbar.

## Elevation & Depth

Depth is a hybrid of tonal layering, borders and restrained shadows. Cards use a subtle edge and no shadow. Floating panels and drawers use the shared float shadow; selected navigation and a few separators use the small card shadow. Focus uses a primary outline and, for focus-ring controls, the shared focus halo.

### Shadow Vocabulary

The sidecar records the resolved light/dark card and float shadows and the theme-bound focus halo. The implementation retains older panel, inset and button shadows for incumbent surfaces; new work should use the observed flat-card and floating-panel roles. Small translucent canvas indicators and image metadata overlays remain local exceptions, not a rule for opaque workbench surfaces.

**The Flat Content Rule.** Content cards remain flat at rest and on hover; floating panels, drawers and focused controls provide the depth vocabulary.

## Shapes

Controls and fields have gently squared corners (button/input roles); panels and image cards use the card/window roles. Thumbnails and badges keep their smaller dedicated roles. One-pixel boundaries distinguish panels and controls. Retain incumbent editor-specific clipping and image mosaics rather than imposing panel radii on artwork.

## Components

### Buttons

Primary submit actions use the filled primary treatment, the primary-foreground token and the larger primary height recorded in frontmatter. Hover darkens via brightness (0.94); disabled actions show reduced opacity and a non-interactive cursor. Secondary controls use opaque surfaces and the dedicated control border; subtle controls use muted text and a quiet hover fill. Feedback uses `--motion-feedback` with the established easing. Shared compact controls are 36px; the generation submit action is 40px. Contextual utility and favorite actions use 32px targets. Favorite actions are named native buttons with aria-pressed; loading disables the action and exposes busy state. Status-only favorite indicators are named images, not controls. Danger actions use the neutral surface and control edge with error-colored ink; do not use an error tint as their general surface.

### Inputs / Fields

Opaque surface, solid ink, dedicated control edge and input radius. Placeholder text uses muted ink at full opacity. Focus shifts the border and adds the shared halo or outline. Labels, validation and asynchronous errors remain visible; failed saves retain editable values. Settings save local configuration and never claim to test a connection.

### Navigation

A 64px-wide, 48px-minimum icon-and-label item sits inside the 80px rail. Default labels use muted text; hover and active states use a surface fill, and active destinations use primary ink. Translated labels wrap without flex compression; extraction uses a localized short label, while accessible names and tooltips retain the full label. Dark-rail focus receives the explicit foreground outline.

### Chips

Metadata chips use subtle fill, foreground ink, badge corners and compact inset spacing. Image overlay badges use local dark translucent fills to remain visible over artwork; do not use those overlays as general form surfaces.

### Cards / Containers

Cards remain opaque, bordered and flat. Image cards clip artwork using the card radius; list rows use the existing smaller control-radius shape. Loading, empty collection, no matches and request failure are distinct states. Delete affects the whole generation record; batch favorite checkpoints successful images and retries remaining failures. Deletion preserves loaded rows and refills shifted pagination boundaries. Contextual image actions and selection controls reveal on hover, keyboard focus or touch devices rather than competing with artwork at rest. Selection uses a real checkbox with a 32px target and 20px visible inner square; selected records keep their selection affordance visible.

### Generation and canvas workbench

Enter inserts a newline; Cmd/Ctrl+Enter submits outside IME composition. Canvas pointer interactions remain immediate and existing Konva state/rendering remains authoritative. Feedback, panel and preview motion use 140ms / 180ms / 200ms tokens; theme transitions use 180ms. Avoid cumulative card staggering and blur exits. CSS and MotionConfig honor reduced motion.

### Preview and modal surfaces

Origin image ID/rect provides a short continuous preview transition where available, with a fade fallback. Keep the thumbnail until the full image loads. Opening preview keeps the list mounted and pauses infinite-load requests. Stack-aware focus management makes Escape close only the top surface and returns focus to its trigger. Closed native disclosures retain their summary in the tab cycle and exclude hidden fields. Preview and edge drawers occupy their full available height; the viewport margin applies only to centered floating dialogs. Busy confirmations cannot dismiss; permanent deletion requires confirmation. Copy success follows successful clipboard write. Generation progress uses backend status and never fabricated percentages.

## Do's and Don'ts

### Do:

- **Do** use semantic theme variables for functional text, controls and surfaces.
- **Do** keep the canvas neutral across all twelve themes and use solid text colors for functional labels.
- **Do** preserve stable drafts and disclosures when a dock becomes a drawer.
- **Do** keep image loading, empty collection, no matches and failure states distinct, with visible retry where applicable.
- **Do** respect reduced motion and use real backend generation status.

### Don't:

- **Don't** add decorative gradients, constant breathing actions or large glass workspaces to the established workbench.
- **Don't** shrink secondary copy below the semantic 12px floor or patch individual font sizes to bypass preferences.
- **Don't** let compact context and parameter drawers overlap, or automatically switch canvas inspector tabs on selection.
- **Don't** report native platform acceptance from browser mocks, or promote verification screenshots and placeholder imagery into shipped assets.

### Verification boundary

Interaction/request contracts, theme contrast and TypeScript/Vite build are checked independently of rendered browser acceptance. Browser verification covers 900×600 / 1200×800 / 1440×900 plus theme and locale smoke checks; mocks stay in verification tooling and never replace production IPC. An earlier native functional workflow passed after unlocking the desktop. The latest polish-specific native check was blocked by the desktop locking again; its native visual acceptance remains pending. Windows and paid provider generation remain untested. See `UI-REVIEW.md` for the current acceptance record; this design document does not certify those checks.

### Provenance

Installed project-local skills (source commits, 2026-11-08):

- pbakaus/impeccable: 924bbd194d02e5e15a227e75b5901fb0d7a4ecad
- nextlevelbuilder/ui-ux-pro-max-skill: 4f2dbb67e64969a9a30ab00742f2d55d66ddec2b
- vercel-labs/agent-skills: 1a2c79d4e6c4a2283dc2c614b6538cbe1ed5ca62
- frontend-design: existing user-installed skill.

UI UX Pro Max lookup supplied contrast/density guidance. Its marketing showcase suggestions are outside this desktop application's brief and were not adopted. Impeccable's Operate guidance and Vercel interface guidelines inform the final review. This merge preserves the established product world and documented behavior; `PRODUCT.md` remains the durable product source.
