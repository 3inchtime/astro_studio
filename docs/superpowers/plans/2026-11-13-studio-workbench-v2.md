# Astro Studio workbench v2 implementation

Approved user plan: 2026-11-13. Base: `68d6e0f` / v0.0.32. Branch: `feat/studio-workbench-v2`.

- [x] Isolate feature branch; preserve all backend/IPC/data contracts.
- [x] Fix pre-existing TrashPage test render race in its own commit.
- [x] Split generation parameters into stable right inspector; compact bottom composer, real task states.
- [x] Add loaded-record batch selection, per-image checkpoints, confirmation, retry and preview origin continuity.
- [x] Reorganize canvas docks, persistent inspector tabs and independent toolbars.
- [x] Simplify project and settings flows; update extraction/trash/common typography.
- [x] Translate all new copy in eight existing flat locale resources; sync document language.
- [x] Run full interaction suite and production build.
- [x] Compare 1000-record gallery / 200-object canvas median p95 over three runs.
- [x] Finish rendered review and retain screenshots at 900/1200/1440px.
- [x] Run isolated macOS WKWebView smoke QA and document partial coverage.
- [x] Complete native macOS shape editing/autosave, settings, preview, file and clipboard functional checks after system unlock, using isolated QA data.
- [x] Complete autonomous visual refinement, current browser screenshots, large-text/locale checks and closed-disclosure keyboard regression.
- [ ] Recheck latest polish in native macOS (desktop locked again before targeted verification).
- [ ] Run Windows WebView2 and real supplier/native file/clipboard integration acceptance on target hardware.

No version bump, push, merge, tag or release is part of this implementation.
