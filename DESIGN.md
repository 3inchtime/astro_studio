# Astro Studio workbench design

## Intent

Operate mode for creation, editing, settings and organization; image-led viewing for previews. Product function and familiar desktop behavior take priority over decorative styling. The approved 2026-11-08 redesign replaces layered glass, tiny low-contrast copy and crowded composer overlays with a coherent workbench.

## Tokens

- Base grid: 4px; controls: 36px, primary actions: 40px; control radius: 8px; panel radius: 12px.
- Body: 14px, secondary: at least 12px, page heading: 20px. Use local platform fonts and existing CJK fallbacks; no network font dependency.
- Base theme: neutral workspace, opaque white panels, blue primary action. All twelve theme IDs remain. Theme factories derive rail/canvas surfaces and correct muted/primary text to 4.5:1 against supported surfaces; dedicated control borders reach 3:1.
- Flat content panels; shadows reserved for overlays and minor separation. No constant breathing call-to-action, decorative text gradients or large glass surfaces.
- 120–180ms feedback, at most 220ms panels; respect reduced motion through CSS and MotionConfig.

## Layout and workflow

- 80px navigation rail with icon and label, grouped creation/project/library destinations. Trash belongs to library, settings at bottom. Existing URLs stay valid.
- Context sidebar collapsible and keyboard-resizable; width preferences versioned. Below 1100px it becomes an overlay. Main surface must fit a 900×600 window.
- Composer: references, text, separate action row. Model/size/count remain accessible; advanced parameters use disclosure. Enter inserts a newline, Cmd/Ctrl+Enter sends outside IME composition.
- Collections: one header/filter/results/detail pattern; loading, failure, no matches and empty collection are distinct. Docked detail at 1280px+, modal drawer below.
- Canvas: 220px documents, flexible stage, 300px inspector at 1100px+; exclusive compact overlays below. Existing editor state and rendering remain authoritative.
- Settings: compact category navigation, consistent field feedback; failed saves retain editable values.

## Interaction contracts

Stack-aware modal focus management; Escape dismisses only the top surface, busy actions cannot dismiss, focus returns to trigger. Input validation and asynchronous errors remain visible with retry. Copy success appears only after successful clipboard write. Permanent deletion requires confirmation. Existing generations use real server status, never fabricated percentages.

## Validation

Vitest for interaction/request contracts and theme contrast, TypeScript/Vite build, real browser screenshots at 900×600 / 1200×800 / 1440×900, every theme and locale smoke checks. Browser mocks must stay in verification tooling and never replace production IPC. Native OS integrations require actual platform runs and must be reported separately.

## Skill sources

Installed project-local skills (source commits, 2026-11-08):
- pbakaus/impeccable: 924bbd194d02e5e15a227e75b5901fb0d7a4ecad
- nextlevelbuilder/ui-ux-pro-max-skill: 4f2dbb67e64969a9a30ab00742f2d55d66ddec2b
- vercel-labs/agent-skills: 1a2c79d4e6c4a2283dc2c614b6538cbe1ed5ca62
- frontend-design: existing user-installed skill.

UI UX Pro Max lookup supplied contrast/density guidance. Its marketing showcase suggestions are outside this desktop application's brief and were not adopted. Impeccable's Operate guidance and Vercel interface guidelines inform the final review.
