# Global UI/UX Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the approved Studio OS design direction across Astro Studio's global shell, creation workflows, management pages, extraction, and settings without changing routes, backend behavior, or data flow.

**Architecture:** Add a compact set of reusable Tailwind v4 utilities in `src/styles/globals.css`, then apply them to existing React components in place. Keep component boundaries stable and use small presentation helpers only where they reduce repeated class strings.

**Tech Stack:** React 19, TypeScript, Tailwind CSS v4, Framer Motion, Vitest, React Testing Library, Tauri IPC wrappers.

---

## File Structure

- Modify `src/styles/globals.css`: global tokens, reusable utilities, focus-visible, reduced-motion, panel/control/card/page-shell utilities.
- Modify `src/components/layout/AppLayout.tsx`: Studio OS app shell, rail, active states, theme popover.
- Modify `src/components/generate/GenerationComposer.tsx`: bottom command surface, parameter controls, focus and disabled states.
- Modify `src/components/generate/GenerationFeed.tsx`: empty state and feed spacing.
- Modify `src/components/generate/MessageBubble.tsx`: studio-log message styling and motion reduction friendliness.
- Modify `src/pages/ProjectsPage.tsx`: shared toolbar, cards, empty/loading/error/status footer.
- Modify `src/pages/GalleryPage.tsx`: page shell and management surface.
- Modify `src/pages/FavoritesPage.tsx`: shared toolbar and prompt favorite cards.
- Modify `src/pages/TrashPage.tsx`: shared toolbar/page shell where present.
- Modify `src/pages/PromptExtractPage.tsx`: creation panel shell, upload/result/history controls.
- Modify `src/pages/SettingsPage.tsx`: page shell, header, tab cards.
- Modify focused child components as needed for consistency: gallery controls, dialogs, selector panels, settings panels.
- Test `src/styles/globals.css` through a lightweight static test.
- Test `src/components/layout/AppLayout.test.tsx` for Studio OS shell classes.
- Test `src/pages/GeneratePage.test.tsx` for the composer command surface.

## Task 1: Global Design Utilities

**Files:**
- Modify: `src/styles/globals.css`
- Create: `src/styles/globals.test.ts`

- [ ] **Step 1: Write failing static tests**

Create `src/styles/globals.test.ts` that reads `src/styles/globals.css` and asserts the new Studio OS utilities exist:

```ts
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

describe("global Studio OS design utilities", () => {
  it("defines reusable control, panel, and focus utilities", () => {
    expect(css).toContain("@utility focus-ring");
    expect(css).toContain("@utility studio-panel");
    expect(css).toContain("@utility studio-control");
    expect(css).toContain("@utility studio-control-primary");
    expect(css).toContain("@utility studio-toolbar");
    expect(css).toContain("@utility studio-card");
  });

  it("removes decorative motion for reduced-motion users", () => {
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).toContain("animation-duration: 0.01ms");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/styles/globals.test.ts`

Expected: FAIL because the Studio OS utilities are not defined yet.

- [ ] **Step 3: Implement global utilities**

Update `src/styles/globals.css`:

- Remove global `letter-spacing: -0.01em` from `body`.
- Add `--color-rail`, `--color-canvas`, `--color-surface-muted`, `--shadow-focus`, and dark-mode equivalents.
- Add `focus-ring`, `studio-panel`, `studio-panel-strong`, `studio-toolbar`, `studio-card`, `studio-control`, `studio-control-primary`, `studio-control-subtle`, `studio-control-danger`, `studio-input`, `studio-empty`, and `studio-status-bar` utilities.
- Add global reduced-motion rules.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/styles/globals.test.ts`

Expected: PASS.

## Task 2: App Shell

**Files:**
- Modify: `src/components/layout/AppLayout.test.tsx`
- Modify: `src/components/layout/AppLayout.tsx`

- [ ] **Step 1: Write failing shell test**

Add an assertion in `AppLayout.test.tsx` that the root shell has `studio-app-shell`, the rail has `studio-nav-rail`, and the theme picker opens inside a `studio-floating-panel`.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/layout/AppLayout.test.tsx`

Expected: FAIL because those semantic class hooks are not present.

- [ ] **Step 3: Implement app shell styling**

Update `AppLayout.tsx`:

- Root uses `studio-app-shell`.
- Rail uses `studio-nav-rail`.
- Nav links and utility buttons use `focus-ring`, pointer cursor, stable hover, and selected surface.
- Logo hover uses border/shadow feedback instead of scale.
- Theme picker uses `studio-floating-panel`.
- Main area uses a subtle studio canvas class.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/components/layout/AppLayout.test.tsx`

Expected: PASS.

## Task 3: Generate Workspace

**Files:**
- Modify: `src/pages/GeneratePage.test.tsx`
- Modify: `src/components/generate/GenerationComposer.tsx`
- Modify: `src/components/generate/GenerationFeed.tsx`
- Modify: `src/components/generate/MessageBubble.tsx`

- [ ] **Step 1: Write failing generate workspace test**

Add an assertion in `GeneratePage.test.tsx` that the generation parameter toolbar uses `studio-toolbar`, and the prompt textbox has an accessible name or label path via its placeholder plus a `studio-input` command surface ancestor.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/GeneratePage.test.tsx`

Expected: FAIL because the composer has not been restyled.

- [ ] **Step 3: Implement generate workspace styling**

Update generation components:

- Composer parameter area uses `studio-toolbar`.
- Composer outer shell uses a named command-surface class and shared panel/control utilities.
- Select controls use `studio-control`.
- Upload, clear, optimize, send, and source-remove buttons use shared control utilities and focus rings.
- Feed empty state uses `studio-empty`.
- Message bubbles use stronger contrast in light/dark and lower layout-shifting motion.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/GeneratePage.test.tsx`

Expected: PASS.

## Task 4: Management Pages

**Files:**
- Modify: `src/pages/ProjectsPage.tsx`
- Modify: `src/pages/GalleryPage.tsx`
- Modify: `src/pages/FavoritesPage.tsx`
- Modify: `src/pages/TrashPage.tsx`
- Modify: gallery child controls as needed.

- [ ] **Step 1: Update management page styling**

Apply shared page and toolbar utilities:

- Projects filter bar uses `studio-toolbar`.
- Project cards use `studio-card`, visible actions, and no full dark hover overlay.
- Gallery, Favorites, and Trash use a consistent page shell.
- Prompt favorite cards use `studio-card`.
- Search/select/view controls use `studio-input` or `studio-control`.

- [ ] **Step 2: Run affected tests**

Run:

```bash
npx vitest run src/pages/ProjectsPage.test.tsx src/pages/GalleryPage.test.tsx src/pages/FavoritesPage.test.tsx src/pages/TrashPage.test.tsx
```

Expected: PASS.

## Task 5: Extraction And Settings

**Files:**
- Modify: `src/pages/PromptExtractPage.tsx`
- Modify: `src/pages/SettingsPage.tsx`
- Modify: settings child panels as needed.

- [ ] **Step 1: Update extraction and settings styling**

Apply shared panel/control utilities:

- Prompt extraction header and sections use `studio-panel`.
- Upload area uses hover/focus-visible treatment.
- Textareas and action buttons use shared utilities.
- Settings header/tabs/panels align to management page styling.
- Logs remain dense and readable.

- [ ] **Step 2: Run affected tests**

Run:

```bash
npx vitest run src/pages/PromptExtractPage.test.tsx src/pages/SettingsPage.test.tsx src/components/settings/ModelSettingsPanel.test.tsx src/components/settings/LlmConfigSection.test.tsx
```

Expected: PASS.

## Task 6: Full Verification And Visual QA

**Files:**
- No planned source edits unless verification finds issues.

- [ ] **Step 1: Run full tests**

Run: `npm test`

Expected: PASS.

- [ ] **Step 2: Run frontend build**

Run: `npm run build`

Expected: PASS.

- [ ] **Step 3: Run local visual smoke check**

Run the Vite dev server with `npm run dev -- --host 127.0.0.1`, then inspect these routes at desktop widths: `/generate`, `/extract`, `/projects`, `/gallery`, `/favorites`, `/settings`.

Expected:

- No blank pages.
- No horizontal scroll at narrow desktop width.
- Major routes share the Studio OS visual language.
- Composer and management toolbars do not collide with long labels.
- Focus states are visible on keyboard traversal.
