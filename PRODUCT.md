# Astro Studio

Astro Studio is a local-first desktop client for AI image creation on macOS and Windows. It connects user-configured image providers and prompt models, stores creations locally, and supports ongoing work through conversations, projects, a canvas, image and prompt collections.

The primary audience is a creator repeatedly generating, inspecting, refining and organizing images. Main success criteria: clear provider/parameter selection, uninterrupted prompt-to-image workflow, readable images and controls, recoverable errors, and predictable keyboard interaction.

The implementation uses Tauri IPC, React 19, Tailwind 4, React Query, Zustand and Konva. It supports eight languages and twelve theme IDs. This UI refresh preserves native window chrome, IPC contracts, data formats, existing routes and user data. There is no new backend, unified Studio data migration, account system or publishing workflow.

User delegated design choices and approved full UI/interaction refresh on 2026-11-08. The chosen interface is a precise image-first desktop workbench; see DESIGN.md.
