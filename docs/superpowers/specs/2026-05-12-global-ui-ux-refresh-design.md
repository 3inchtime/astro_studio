# Astro Studio Global UI/UX Refresh Design

Date: 2026-05-12

## Overview

This redesign updates Astro Studio from a set of locally polished screens into a cohesive desktop creative studio. The target is a professional image-generation workspace: dense enough for repeated daily use, calm enough for long sessions, and tactile enough to feel native on macOS and Windows.

The confirmed direction is **Studio OS**: a mature creative control surface built from warm neutral surfaces, precise blue-violet accents, consistent glass where it has functional value, and motion that clarifies state instead of decorating the interface.

## Goals

1. Establish one global visual language across navigation, creation, management, extraction, and settings.
2. Improve hierarchy and scanability for high-frequency workflows.
3. Make hover, active, disabled, focus-visible, loading, and empty states consistent.
4. Preserve the current information architecture, routes, IPC API, data model, and i18n structure.
5. Keep the application feeling like a desktop productivity tool, not a marketing landing page.

## Non-Goals

1. No backend behavior changes.
2. No route changes or new navigation concepts.
3. No new persistence model for user layout preferences.
4. No replacement of the existing Tailwind v4 design system.
5. No decorative full-page hero sections, oversized marketing copy, emoji icons, or purely ornamental gradient blobs.

## Design System

### Visual Direction

Astro Studio keeps the existing warm stone base and blue-violet identity, but the refresh makes those choices stricter:

- Surfaces use a small set of elevation levels: app canvas, rail/sidebar, panels, cards, floating overlays.
- Blue-violet appears primarily in selected states, primary actions, focus rings, and generation-related affordances.
- Glass treatment is reserved for overlays, composer shells, and panels that sit over the app canvas. Regular cards use opaque or nearly opaque surfaces.
- Shadows become less fuzzy and more structural: subtle card shadow, stronger floating shadow, and a panel shadow for sticky bottom surfaces.
- Typography stays compact. Large type is only used for real page headers, not cards, toolbars, or settings panels.

### Global Tokens

Update `src/styles/globals.css` with reusable tokens and utilities:

- Radius: keep cards and controls at 8-12px, with floating overlays allowed up to 16px.
- Focus: introduce a reusable `focus-ring` utility using `focus-visible`.
- Controls: introduce reusable button/control surface utilities for neutral, primary, subtle, and danger states.
- Motion: keep 150-300ms transitions for hover and control feedback. Use longer motion only for page or modal entrance.
- Reduced motion: add a global `prefers-reduced-motion` block that removes decorative animation and keeps state changes readable.
- Text: remove global negative letter spacing. Use explicit tracking only for small uppercase labels.

### Interaction Rules

- Every clickable element must have a visible hover state and pointer cursor.
- Keyboard focus must be visible without relying on hover.
- Disabled controls must use both opacity and cursor treatment.
- Hover effects must not shift layout. Use color, border, opacity, and shadow changes before transforms.
- Infinite animation is allowed for active loading indicators only.
- Icons remain Lucide; no emoji are used as UI icons.

## App Shell

### Left Navigation Rail

The rail becomes a stable desktop dock:

- Use a slightly stronger background than the app canvas.
- Active route receives a filled selected state plus the existing left indicator.
- Nav buttons use consistent 40px hit areas, pointer cursor, focus-visible ring, and tooltip/title behavior.
- Logo hover should not rely on scale; use shadow or border feedback to avoid layout jitter.

### Context Sidebar

Conversation and project sidebars keep their current widths and resize behavior, but receive the same surface, border, header, search, and item-state treatment as the rest of the app.

### Main Canvas

The main canvas keeps a restrained ambient background. It should not introduce decorative blobs. Pages may use full-width bands or constrained work areas depending on workflow density.

## Creation Workflows

### Generate Page

The generate page is the primary creative workspace.

- Message feed keeps the current chat model, but spacing and bubble treatment should feel more like a studio log than a consumer chat app.
- Empty state should be calm and actionable without becoming a landing page.
- User bubbles use a high-contrast readable surface in both light and dark mode.
- Assistant image outputs keep strong image presentation but reduce excessive blur/scale entrance.
- Processing state keeps the custom loading scene but respects reduced motion.
- The composer becomes a clear bottom command surface: parameters, source images, text input, optimization, and send action should read as one tool.
- Parameter controls should support many labels without text collision. When space is constrained, labels truncate cleanly and the row remains stable.

### Prompt Extraction Page

Prompt extraction should match the creation surface language:

- Upload, result, and history sections use the same panel shell.
- Drop/upload area remains visibly interactive with strong focus-visible and hover states.
- Result textarea and action row use the same control styling as the generation composer.
- History cards become denser and more consistent with gallery cards.

## Content Management Workflows

### Projects

Projects should feel like a collection manager:

- Top filter bar uses the shared toolbar pattern.
- Search input and create button use shared controls.
- Project cards keep the mosaic preview but reduce heavy hover overlay treatment. Actions should be discoverable without hiding the whole card under a dark overlay.
- Empty, loading, and error states use the shared empty-state component language.
- Stats footer uses a quieter status-bar style.

### Gallery, Favorites, Trash

These pages should use one management-page frame:

- Top toolbar: title or context, filter controls, search, view mode, and reset actions share spacing and control states.
- Grids use consistent card borders, hover states, focus handling, and lazy images.
- Detail panels keep their existing behavior but align overlay surfaces, shadow, and close/action controls with the new floating-panel rules.
- Prompt favorites use card styling consistent with projects and gallery, with action buttons that stay readable on small widths.

## Settings

Settings stays operational and dense:

- Header and tabs use the same page header and selectable-card language as management pages.
- General, model, and logs panels receive shared field, button, and status treatment.
- Logs remain information-dense; avoid decorative styling that reduces readability.
- Saved/error/warning states use semantic colors consistently.

## Component Strategy

Prefer targeted reusable utilities over a large component rewrite:

1. Add CSS utilities for focus, controls, panels, and page shells in `globals.css`.
2. Apply these utilities across existing components without changing data flow.
3. Extract a tiny shared page shell only if repeated markup becomes noisy during implementation.
4. Keep component file boundaries intact unless a file already touched by the refresh becomes difficult to reason about.

Expected high-impact files:

- `src/styles/globals.css`
- `src/components/layout/AppLayout.tsx`
- `src/components/sidebar/ConversationList.tsx`
- `src/components/generate/GenerationComposer.tsx`
- `src/components/generate/GenerationFeed.tsx`
- `src/components/generate/MessageBubble.tsx`
- `src/pages/ProjectsPage.tsx`
- `src/pages/GalleryPage.tsx`
- `src/pages/FavoritesPage.tsx`
- `src/pages/TrashPage.tsx`
- `src/pages/PromptExtractPage.tsx`
- `src/pages/SettingsPage.tsx`
- Settings, gallery, projects, common dialog, and selector components as needed for consistency.

## Accessibility

The refresh must improve keyboard and low-motion behavior:

- Add `focus-visible` rings to buttons, links, inputs, selects, textareas, and clickable cards.
- Preserve labels and ARIA attributes already present.
- Do not remove native semantics from buttons, links, selects, or textareas.
- Use color plus border/icon/shape for selected and error states where possible.
- Add or preserve `disabled:cursor-not-allowed` and visible disabled contrast.
- Respect `prefers-reduced-motion` for Framer Motion where practical and globally for CSS animations.

## Testing And Verification

Implementation should be verified with:

1. `npm test`
2. `npm run build`
3. Browser checks at representative desktop widths for:
   - `/generate`
   - `/extract`
   - `/projects`
   - `/gallery`
   - `/favorites`
   - `/settings`
4. Visual checks for light and dark themes.
5. Responsive checks for narrow desktop/small tablet widths where toolbars and composer controls are most likely to collide.
6. Focus traversal spot checks using keyboard only.

## Delivery Criteria

The refresh is complete when:

- The major routes share one visual language.
- Common controls have consistent hover, active, disabled, and focus-visible behavior.
- The generate composer and management toolbars remain stable with long translated labels.
- Dark and light themes both maintain readable contrast.
- No new decorative-only animation runs indefinitely.
- Existing tests and frontend build pass.
