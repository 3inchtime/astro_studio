# Project Home and Gallery Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a top-level `Projects` section with a dedicated project homepage, remove project UI from the always-visible conversation sidebar, and simplify image filtering to prompt text, model, and date.

**Architecture:** Keep the existing Generate workspace as the place where prompts are created and conversation threads are continued, but make `AppLayout` route-aware so project routes render project navigation instead of the mixed project/conversation sidebar. Reuse the existing gallery grid and detail panel in both the global gallery and the new project homepage, and extend the search pipeline with an optional `projectId` so project pages can query only their own generations.

**Tech Stack:** Tauri 2, React 19, react-router-dom v7, TypeScript, Vitest, Rust, rusqlite

---

### File Structure

**Files and responsibilities:**

- Create: `src/components/projects/ProjectsSidebar.tsx`
  - route-specific sidebar for `/projects` routes; lists non-default projects and creates new ones
- Create: `src/components/projects/ProjectSummaryCards.tsx`
  - overview metrics for a single project homepage
- Create: `src/components/projects/ProjectImagePanel.tsx`
  - project-scoped image grid with simplified filters, paging, and image detail state
- Create: `src/pages/ProjectsPage.tsx`
  - projects directory view for `/projects`
- Create: `src/pages/ProjectHomePage.tsx`
  - homepage for `/projects/:projectId` showing overview, recent conversations, and images
- Create: `src/pages/ProjectsPage.test.tsx`
  - verifies the projects directory and hidden default project behavior
- Create: `src/pages/ProjectHomePage.test.tsx`
  - verifies project homepage loading, empty states, and project-scoped image search
- Create: `src/components/layout/AppLayout.test.tsx`
  - verifies route-aware sidebar switching
- Create: `src/components/sidebar/ConversationList.test.tsx`
  - verifies the conversation list no longer renders project strips and shows project context when scoped
- Modify: `src/App.tsx`
  - register `/projects` and `/projects/:projectId`
- Modify: `src/components/layout/AppLayout.tsx`
  - add top-level `Projects` nav item and switch the secondary sidebar by route
- Modify: `src/components/sidebar/ConversationList.tsx`
  - remove visible project UI, keep conversation list, and show project context header when `activeProjectId` is set
- Modify: `src/components/gallery/GallerySearchBar.tsx`
  - continue rendering the shared prompt/model/date search shell for both gallery and project pages
- Modify: `src/lib/galleryFilterConfig.ts`
  - reduce filter config to model plus date range
- Modify: `src/lib/galleryFilters.ts`
  - keep compact/active/update helpers aligned with the simplified filter shape
- Modify: `src/lib/galleryFilters.test.ts`
  - verify the reduced filter config and active-state behavior
- Modify: `src/pages/GalleryPage.tsx`
  - keep global gallery wired to the simplified filters
- Modify: `src/pages/GalleryPage.test.tsx`
  - verify removed filters are gone and the remaining ones still search correctly
- Modify: `src/lib/api.ts`
  - extend `searchGenerations` to accept an optional `projectId`
- Modify: `src/lib/api.test.ts`
  - verify the IPC payload for simplified filters and `projectId`
- Modify: `src/types/index.ts`
  - simplify `GenerationSearchFilters`; add `image_count` to `Project`
- Modify: `src/locales/en.json`
  - add English strings for `nav.projects` and the new project pages
- Modify: `src/locales/zh-CN.json`
  - add Simplified Chinese strings for the same project keys
- Modify: `src-tauri/src/models.rs`
  - simplify `GenerationSearchFilters`; add `image_count` to `Project`
- Modify: `src-tauri/src/gallery.rs`
  - simplify SQL filter building and add optional project scoping
- Modify: `src-tauri/src/lib.rs`
  - extend `projects_base_sql` with `image_count`

### Task 1: Simplify the shared gallery filter model

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/lib/galleryFilterConfig.ts`
- Modify: `src/lib/galleryFilters.ts`
- Modify: `src/components/gallery/GallerySearchBar.tsx`
- Modify: `src/pages/GalleryPage.tsx`
- Test: `src/lib/galleryFilters.test.ts`
- Test: `src/pages/GalleryPage.test.tsx`

- [ ] **Step 1: Write the failing tests**

Add this test to `src/lib/galleryFilters.test.ts`:

```ts
it("builds gallery search config with only model and date fields", () => {
  const filters: GenerationSearchFilters = {
    model: "gpt-image-2",
    created_from: "2026-05-01",
    created_to: "2026-05-31",
  };

  const config = createGallerySearchConfig((key) => key, filters, () => undefined);

  expect(config.fields.map((field) => field.key)).toEqual([
    "model",
    "created_from",
    "created_to",
  ]);
});
```

Replace the advanced-filter test in `src/pages/GalleryPage.test.tsx` with:

```ts
it("searches with prompt, model, and date only", async () => {
  render(<GalleryPage />);

  await waitFor(() => {
    expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, undefined);
  });

  fireEvent.change(screen.getByLabelText("gallery.filterModel"), {
    target: { value: "gpt-image-2" },
  });
  fireEvent.change(screen.getByLabelText("gallery.filterCreatedFrom"), {
    target: { value: "2026-05-01" },
  });
  fireEvent.change(screen.getByPlaceholderText("gallery.search"), {
    target: { value: "sunrise" },
  });
  fireEvent.click(screen.getByRole("button", { name: "gallery.applyFilters" }));

  await waitFor(() => {
    expect(searchGenerations).toHaveBeenLastCalledWith("sunrise", 1, false, {
      model: "gpt-image-2",
      created_from: "2026-05-01",
    }, undefined);
  });

  expect(screen.queryByLabelText("gallery.filterStatus")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("gallery.filterQuality")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("gallery.filterSources")).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
npm test -- src/lib/galleryFilters.test.ts src/pages/GalleryPage.test.tsx
```

Expected: FAIL because `createGallerySearchConfig` still emits the full advanced filter list and `searchGenerations` still uses the old four-argument shape in the tests.

- [ ] **Step 3: Write the minimal implementation**

Update `src/types/index.ts` so the frontend filter type becomes:

```ts
export interface GenerationSearchFilters {
  model?: ImageModel | "";
  created_from?: string;
  created_to?: string;
}

export interface Project {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
  archived_at: string | null;
  pinned_at: string | null;
  deleted_at: string | null;
  conversation_count: number;
  image_count: number;
}
```

Trim `src/lib/galleryFilterConfig.ts` down to:

```ts
export function createGallerySearchConfig(
  t: TranslationFn,
  filters: GenerationSearchFilters,
  onFilterChange: <K extends keyof GenerationSearchFilters>(
    key: K,
    value: GenerationSearchFilters[K],
  ) => void,
): GallerySearchConfig {
  return {
    title: t("gallery.title"),
    searchPlaceholder: t("gallery.search"),
    applyFilters: t("gallery.applyFilters"),
    resetFilters: t("gallery.resetFilters"),
    fields: [
      selectField(
        "model",
        t("gallery.filterModel"),
        filters.model ?? "",
        [
          { value: "", label: "All models" },
          ...IMAGE_MODEL_CATALOG.map((entry) => ({
            value: entry.id,
            label: entry.label,
          })),
        ],
        (value) => onFilterChange("model", value),
      ),
      dateField(
        "created_from",
        t("gallery.filterCreatedFrom"),
        filters.created_from ?? "",
        (value) => onFilterChange("created_from", value),
      ),
      dateField(
        "created_to",
        t("gallery.filterCreatedTo"),
        filters.created_to ?? "",
        (value) => onFilterChange("created_to", value),
      ),
    ],
  };
}
```

Keep `compactFilters` in `src/lib/galleryFilters.ts` narrow and unchanged in spirit:

```ts
export function compactFilters(
  filters: GenerationSearchFilters,
): GenerationSearchFilters {
  return Object.fromEntries(
    Object.entries(filters).filter(([, value]) => value !== "" && value !== undefined),
  ) as GenerationSearchFilters;
}
```

Update `src/pages/GalleryPage.tsx` so the search call signature includes the new optional fifth argument:

```ts
const result = await searchGenerations(
  nextQuery.trim() || undefined,
  pageToLoad,
  false,
  compactFilters(nextFilters),
  undefined,
);
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:

```bash
npm test -- src/lib/galleryFilters.test.ts src/pages/GalleryPage.test.tsx
```

Expected: PASS with only `model`, `created_from`, and `created_to` rendered and searched.

- [ ] **Step 5: Commit**

```bash
git add src/types/index.ts src/lib/galleryFilterConfig.ts src/lib/galleryFilters.ts src/lib/galleryFilters.test.ts src/components/gallery/GallerySearchBar.tsx src/pages/GalleryPage.tsx src/pages/GalleryPage.test.tsx
git commit -m "refactor: simplify gallery filter surface"
```

### Task 2: Extend generation search with optional project scope

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/lib/api.test.ts`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/gallery.rs`

- [ ] **Step 1: Write the failing tests**

Replace the gallery IPC test in `src/lib/api.test.ts` with:

```ts
it("forwards simplified gallery filters and optional project scope through Tauri IPC", async () => {
  tauriApi.invoke.mockResolvedValue({
    generations: [],
    total: 0,
    page: 1,
    page_size: 20,
  });

  await searchGenerations(
    "sunrise",
    2,
    false,
    {
      model: "gpt-image-2",
      created_from: "2026-05-01",
      created_to: "2026-05-31",
    },
    "project-1",
  );

  expect(tauriApi.invoke).toHaveBeenCalledWith("search_generations", {
    query: "sunrise",
    page: 2,
    onlyDeleted: null,
    filters: {
      model: "gpt-image-2",
      created_from: "2026-05-01",
      created_to: "2026-05-31",
    },
    projectId: "project-1",
  });
});
```

Add this unit test at the bottom of `src-tauri/src/gallery.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_filters_to_sql_adds_project_scope_clause() {
        let (sql, params) = generation_filters_to_sql(
            false,
            Some("sunrise"),
            Some(&GenerationSearchFilters {
                model: Some("gpt-image-2".into()),
                created_from: Some("2026-05-01".into()),
                created_to: None,
            }),
            Some("project-1"),
        );

        assert!(sql.contains("EXISTS (SELECT 1 FROM conversations c"));
        assert!(sql.contains("COALESCE(c.project_id, 'default')"));
        assert_eq!(params.len(), 3);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
npm test -- src/lib/api.test.ts
cargo test generation_filters_to_sql_adds_project_scope_clause
```

Expected: FAIL because `searchGenerations` does not accept `projectId` yet and `generation_filters_to_sql` still takes only three arguments.

- [ ] **Step 3: Write the minimal implementation**

Update `src/lib/api.ts`:

```ts
export async function searchGenerations(
  query?: string,
  page?: number,
  onlyDeleted?: boolean,
  filters?: GenerationSearchFilters,
  projectId?: string | null,
): Promise<SearchResult> {
  return invoke("search_generations", {
    query: query || null,
    page,
    onlyDeleted: onlyDeleted || null,
    filters: filters || null,
    projectId: projectId || null,
  });
}
```

Update `src-tauri/src/models.rs` so the Rust filter struct matches the simplified shape:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerationSearchFilters {
    pub model: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
}
```

Update the SQL builder in `src-tauri/src/gallery.rs`:

```rust
fn generation_filters_to_sql(
    only_deleted: bool,
    query: Option<&str>,
    filters: Option<&GenerationSearchFilters>,
    project_id: Option<&str>,
) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut clauses = vec![if only_deleted {
        deleted_generation_filter("g")
    } else {
        active_generation_filter("g")
    }];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
        clauses.push(format!("g.prompt LIKE ?{}", params.len() + 1));
        params.push(Box::new(format!("%{}%", query)));
    }

    if let Some(project_id) = project_id.map(str::trim).filter(|value| !value.is_empty()) {
        clauses.push(format!(
            "EXISTS (SELECT 1 FROM conversations c WHERE c.id = g.conversation_id AND COALESCE(c.project_id, 'default') = ?{})",
            params.len() + 1
        ));
        params.push(Box::new(project_id.to_string()));
    }

    if let Some(filters) = filters {
        generation_search_value_clause(&mut clauses, &mut params, "g.engine", filters.model.as_deref());
        generation_search_range_clause(
            &mut clauses,
            &mut params,
            "g.created_at",
            filters.created_from.as_deref(),
            filters.created_to.as_deref(),
        );
    }

    (format!("WHERE {}", clauses.join(" AND ")), params)
}
```

Update the Tauri command signature:

```rust
#[tauri::command]
pub(crate) fn search_generations(
    db: tauri::State<'_, Database>,
    query: Option<String>,
    page: Option<i32>,
    only_deleted: Option<bool>,
    filters: Option<GenerationSearchFilters>,
    project_id: Option<String>,
) -> Result<SearchResult, String> {
    let (where_sql, params_boxed) = generation_filters_to_sql(
        only_deleted.unwrap_or(false),
        query.as_deref(),
        filters.as_ref(),
        project_id.as_deref(),
    );
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:

```bash
npm test -- src/lib/api.test.ts
cargo test generation_filters_to_sql_adds_project_scope_clause
```

Expected: PASS, with the IPC payload carrying `projectId` and the Rust SQL builder generating an `EXISTS` clause against `conversations`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/api.ts src/lib/api.test.ts src-tauri/src/models.rs src-tauri/src/gallery.rs
git commit -m "feat: add project-scoped generation search"
```

### Task 3: Add the top-level Projects route and route-aware layout shell

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/layout/AppLayout.tsx`
- Create: `src/components/layout/AppLayout.test.tsx`
- Create: `src/components/projects/ProjectsSidebar.tsx`
- Modify: `src/locales/en.json`
- Modify: `src/locales/zh-CN.json`

- [ ] **Step 1: Write the failing test**

Create `src/components/layout/AppLayout.test.tsx`:

```tsx
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import AppLayout from "./AppLayout";

vi.mock("../sidebar/ConversationList", () => ({
  default: () => <div data-testid="conversation-sidebar" />,
}));

vi.mock("../projects/ProjectsSidebar", () => ({
  default: () => <div data-testid="projects-sidebar" />,
}));

describe("AppLayout", () => {
  it("renders the project sidebar on project routes and the conversation sidebar elsewhere", () => {
    const { rerender } = render(
      <MemoryRouter initialEntries={["/projects"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/projects" element={<div>projects</div>} />
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId("projects-sidebar")).toBeInTheDocument();
    expect(screen.queryByTestId("conversation-sidebar")).not.toBeInTheDocument();

    rerender(
      <MemoryRouter initialEntries={["/generate"]}>
        <Routes>
          <Route element={<AppLayout />}>
            <Route path="/projects" element={<div>projects</div>} />
            <Route path="/generate" element={<div>generate</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId("conversation-sidebar")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
npm test -- src/components/layout/AppLayout.test.tsx
```

Expected: FAIL because `AppLayout` still always renders `ConversationList`, there is no `/projects` route, and `ProjectsSidebar` does not exist.

- [ ] **Step 3: Write the minimal implementation**

Add the routes in `src/App.tsx`:

```tsx
<Route element={<AppLayout />}>
  <Route path="/generate" element={<GeneratePage />} />
  <Route path="/projects" element={<ProjectsPage />} />
  <Route path="/projects/:projectId" element={<ProjectHomePage />} />
  <Route path="/gallery" element={<GalleryPage />} />
  <Route path="/trash" element={<TrashPage />} />
  <Route path="/favorites" element={<FavoritesPage />} />
  <Route path="/settings" element={<SettingsPage />} />
  <Route path="*" element={<Navigate to="/generate" replace />} />
</Route>
```

Add the new nav item and sidebar switch in `src/components/layout/AppLayout.tsx`:

```tsx
import { FolderKanban, Image, Settings, Sparkles, Sun, Moon, Heart } from "lucide-react";
import ProjectsSidebar from "../projects/ProjectsSidebar";

const navItems = [
  { to: "/generate", icon: Sparkles, labelKey: "nav.generate" },
  { to: "/projects", icon: FolderKanban, labelKey: "nav.projects" },
  { to: "/gallery", icon: Image, labelKey: "nav.gallery" },
  { to: "/favorites", icon: Heart, labelKey: "nav.favorites" },
];

const isProjectsRoute =
  location.pathname === "/projects" || location.pathname.startsWith("/projects/");
```

Render the proper sidebar:

```tsx
<aside
  className="flex shrink-0 flex-col border-r border-border-subtle"
  style={{ width: widths[0] }}
>
  {isProjectsRoute ? (
    <ProjectsSidebar
      activeProjectId={activeProjectId}
      onSelectProject={(id) => {
        setActiveProjectId(id);
        setActiveConversationId(null);
        navigate(id ? `/projects/${id}` : "/projects");
      }}
      onProjectCreated={(id) => {
        setActiveProjectId(id);
        setActiveConversationId(null);
        navigate(`/projects/${id}`);
      }}
    />
  ) : (
    <ConversationList
      activeProjectId={activeProjectId}
      activeConversationId={activeConversationId}
      refreshKey={conversationRefreshKey}
      onSelectProject={selectProject}
      onProjectCreated={selectCreatedProject}
      onSelectConversation={selectConversation}
      onInitialConversation={selectInitialConversation}
      onNewConversation={createNewConversation}
    />
  )}
</aside>
```

Create a stub-safe `src/components/projects/ProjectsSidebar.tsx`:

```tsx
import { useEffect, useState } from "react";
import { FolderKanban, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { createProject, getProjects } from "../../lib/api";
import type { Project } from "../../types";

export default function ProjectsSidebar({
  activeProjectId,
  onSelectProject,
  onProjectCreated,
}: {
  activeProjectId: string | null;
  onSelectProject: (id: string | null) => void;
  onProjectCreated: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [projects, setProjects] = useState<Project[]>([]);

  useEffect(() => {
    getProjects(false).then((items) => {
      setProjects(items.filter((project) => project.id !== "default"));
    }).catch(() => {
      setProjects([]);
    });
  }, []);

  async function handleCreateProject() {
    const name = window.prompt(t("sidebar.newProject"));
    if (!name?.trim()) return;
    const project = await createProject(name.trim());
    onProjectCreated(project.id);
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between px-4 py-4">
        <div className="flex items-center gap-2">
          <FolderKanban size={14} />
          <span>{t("nav.projects")}</span>
        </div>
        <button onClick={handleCreateProject} aria-label={t("sidebar.newProject")}>
          <Plus size={14} />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto px-2">
        <button
          className={activeProjectId === null ? "bg-primary/8 text-primary" : ""}
          onClick={() => onSelectProject(null)}
        >
          {t("projects.directory")}
        </button>
        {projects.map((project) => (
          <button
            key={project.id}
            className={activeProjectId === project.id ? "bg-primary/8 text-primary" : ""}
            onClick={() => onSelectProject(project.id)}
          >
            {project.name}
          </button>
        ))}
      </div>
    </div>
  );
}
```

Add locale keys:

```json
"nav.projects": "Projects",
"projects.directory": "Projects"
```

```json
"nav.projects": "项目",
"projects.directory": "项目"
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
npm test -- src/components/layout/AppLayout.test.tsx
```

Expected: PASS with `/projects` rendering `ProjectsSidebar` and `/generate` still rendering `ConversationList`.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/components/layout/AppLayout.tsx src/components/layout/AppLayout.test.tsx src/components/projects/ProjectsSidebar.tsx src/locales/en.json src/locales/zh-CN.json
git commit -m "feat: add projects route shell"
```

### Task 4: Build the projects directory and expose image counts

**Files:**
- Create: `src/pages/ProjectsPage.tsx`
- Test: `src/pages/ProjectsPage.test.tsx`
- Modify: `src/types/index.ts`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/locales/en.json`
- Modify: `src/locales/zh-CN.json`

- [ ] **Step 1: Write the failing test**

Create `src/pages/ProjectsPage.test.tsx`:

```tsx
import { MemoryRouter } from "react-router-dom";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectsPage from "./ProjectsPage";

const getProjects = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("../lib/api", () => ({
  getProjects: (...args: unknown[]) => getProjects(...args),
}));

describe("ProjectsPage", () => {
  beforeEach(() => {
    getProjects.mockReset();
    getProjects.mockResolvedValue([
      {
        id: "default",
        name: "Default Project",
        created_at: "",
        updated_at: "",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        conversation_count: 2,
        image_count: 5,
      },
      {
        id: "project-1",
        name: "Brand Storyboards",
        created_at: "",
        updated_at: "2026-05-07T01:00:00Z",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        conversation_count: 12,
        image_count: 86,
      },
    ]);
  });

  it("shows user-facing projects and hides the default project", async () => {
    render(
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(getProjects).toHaveBeenCalledWith(false);
    });

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.getByText("86 images")).toBeInTheDocument();
    expect(screen.queryByText("Default Project")).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
npm test -- src/pages/ProjectsPage.test.tsx
```

Expected: FAIL because `ProjectsPage` does not exist and `Project` does not expose `image_count`.

- [ ] **Step 3: Write the minimal implementation**

Extend the Rust project model in `src-tauri/src/models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    pub pinned_at: Option<String>,
    pub deleted_at: Option<String>,
    pub conversation_count: i32,
    pub image_count: i32,
}
```

Update `projects_base_sql` and `row_to_project` in `src-tauri/src/lib.rs`:

```rust
fn projects_base_sql(where_sql: &str) -> String {
    format!(
        "SELECT p.id, p.name, p.created_at, p.updated_at, p.archived_at, p.pinned_at, p.deleted_at, \
         (SELECT COUNT(*) FROM conversations c \
          WHERE c.project_id = p.id AND c.deleted_at IS NULL AND c.archived_at IS NULL) as conversation_count, \
         (SELECT COUNT(i.id) FROM conversations c \
          JOIN generations g ON g.conversation_id = c.id \
          JOIN images i ON i.generation_id = g.id \
          WHERE c.project_id = p.id AND c.deleted_at IS NULL AND c.archived_at IS NULL AND g.deleted_at IS NULL) as image_count \
         FROM projects p \
         WHERE {} \
         ORDER BY CASE WHEN p.pinned_at IS NULL THEN 1 ELSE 0 END, p.pinned_at DESC, p.updated_at DESC",
        where_sql
    )
}

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        archived_at: row.get("archived_at")?,
        pinned_at: row.get("pinned_at")?,
        deleted_at: row.get("deleted_at")?,
        conversation_count: row.get("conversation_count")?,
        image_count: row.get("image_count")?,
    })
}
```

Create `src/pages/ProjectsPage.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { createProject, getProjects } from "../lib/api";
import type { Project } from "../types";

export default function ProjectsPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [projects, setProjects] = useState<Project[]>([]);

  useEffect(() => {
    getProjects(false).then((items) => {
      setProjects(items.filter((project) => project.id !== "default"));
    }).catch(() => {
      setProjects([]);
    });
  }, []);

  async function handleCreateProject() {
    const name = window.prompt(t("sidebar.newProject"));
    if (!name?.trim()) return;
    const project = await createProject(name.trim());
    navigate(`/projects/${project.id}`);
  }

  return (
    <div className="h-full overflow-y-auto px-8 py-8">
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-[28px] font-semibold text-foreground">{t("projects.title")}</h1>
        <button
          onClick={handleCreateProject}
          className="rounded-[12px] bg-primary px-4 py-2 text-[12px] font-medium text-white"
        >
          {t("sidebar.newProject")}
        </button>
      </div>
      <div className="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        {projects.map((project) => (
          <button
            key={project.id}
            onClick={() => navigate(`/projects/${project.id}`)}
            className={`rounded-[14px] border p-5 text-left shadow-card transition-transform hover:-translate-y-0.5 ${
              project.pinned_at ? "border-primary/30 bg-primary/6" : "border-border-subtle bg-surface"
            }`}
          >
            <div className="text-[16px] font-semibold text-foreground">{project.name}</div>
            <div className="mt-2 text-[12px] text-muted">
              {project.conversation_count} conversations
            </div>
            <div className="mt-1 text-[12px] text-muted">
              {project.image_count} images
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
```

Add copy:

```json
"projects.title": "Projects"
```

```json
"projects.title": "项目"
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
npm test -- src/pages/ProjectsPage.test.tsx
cargo check
```

Expected: PASS, with `ProjectsPage` hiding `default` and showing real `image_count` values from `getProjects`.

- [ ] **Step 5: Commit**

```bash
git add src/pages/ProjectsPage.tsx src/pages/ProjectsPage.test.tsx src/types/index.ts src-tauri/src/models.rs src-tauri/src/lib.rs src/locales/en.json src/locales/zh-CN.json
git commit -m "feat: add projects directory page"
```

### Task 5: Build the project homepage with overview, recent conversations, and project images

**Files:**
- Create: `src/components/projects/ProjectSummaryCards.tsx`
- Create: `src/components/projects/ProjectImagePanel.tsx`
- Create: `src/pages/ProjectHomePage.tsx`
- Test: `src/pages/ProjectHomePage.test.tsx`
- Modify: `src/locales/en.json`
- Modify: `src/locales/zh-CN.json`

- [ ] **Step 1: Write the failing test**

Create `src/pages/ProjectHomePage.test.tsx`:

```tsx
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectHomePage from "./ProjectHomePage";

const getProjects = vi.fn();
const getConversations = vi.fn();
const searchGenerations = vi.fn();
const setActiveConversationId = vi.fn();
const setActiveProjectId = vi.fn();
const navigate = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("../lib/api", () => ({
  getProjects: (...args: unknown[]) => getProjects(...args),
  getConversations: (...args: unknown[]) => getConversations(...args),
  searchGenerations: (...args: unknown[]) => searchGenerations(...args),
}));

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    setActiveConversationId,
    setActiveProjectId,
  }),
}));

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => navigate,
  };
});

describe("ProjectHomePage", () => {
  beforeEach(() => {
    getProjects.mockReset();
    getConversations.mockReset();
    searchGenerations.mockReset();
    navigate.mockReset();
    setActiveConversationId.mockReset();
    setActiveProjectId.mockReset();

    getProjects.mockResolvedValue([
      {
        id: "project-1",
        name: "Brand Storyboards",
        created_at: "",
        updated_at: "2026-05-07T01:00:00Z",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        conversation_count: 12,
        image_count: 86,
      },
    ]);
    getConversations.mockResolvedValue([
      {
        id: "conversation-1",
        project_id: "project-1",
        title: "Homepage hero direction",
        created_at: "",
        updated_at: "2026-05-07T01:00:00Z",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        generation_count: 8,
        latest_generation_at: "2026-05-07T01:00:00Z",
        latest_thumbnail: null,
      },
    ]);
    searchGenerations.mockResolvedValue({
      generations: [],
      total: 0,
      page: 1,
      page_size: 20,
    });
  });

  it("loads the project overview, recent conversations, and project-scoped images", async () => {
    render(
      <MemoryRouter initialEntries={["/projects/project-1"]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectHomePage />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.getByText("projects.recentConversations")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "projects.manage" })).toBeInTheDocument();

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, "project-1");
    });

    expect(screen.getByText("86")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
npm test -- src/pages/ProjectHomePage.test.tsx
```

Expected: FAIL because `ProjectHomePage` and the project-specific image panel do not exist yet.

- [ ] **Step 3: Write the minimal implementation**

Create `src/components/projects/ProjectSummaryCards.tsx`:

```tsx
import type { Project } from "../../types";

export default function ProjectSummaryCards({
  project,
  recentModels,
}: {
  project: Project;
  recentModels: string[];
}) {
  return (
    <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">Scope</div>
        <div className="mt-2 text-[24px] font-semibold text-foreground">{project.conversation_count}</div>
        <div className="text-[12px] text-muted">active conversations</div>
      </div>
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">Output</div>
        <div className="mt-2 text-[24px] font-semibold text-foreground">{project.image_count}</div>
        <div className="text-[12px] text-muted">saved images</div>
      </div>
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">Models</div>
        <div className="mt-2 text-[14px] font-semibold text-foreground">
          {recentModels.join(", ") || "None yet"}
        </div>
      </div>
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">Updated</div>
        <div className="mt-2 text-[14px] font-semibold text-foreground">{project.updated_at}</div>
      </div>
    </div>
  );
}
```

Create `src/components/projects/ProjectImagePanel.tsx`:

```tsx
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { GenerationResult, GenerationSearchFilters } from "../../types";
import GallerySearchBar from "../gallery/GallerySearchBar";
import GenerationGrid from "../gallery/GenerationGrid";
import EmptyCollectionState from "../gallery/EmptyCollectionState";
import PaginationControls from "../gallery/PaginationControls";
import { createGallerySearchConfig } from "../../lib/galleryFilterConfig";
import { compactFilters, isFilterActive, updateFilterValue } from "../../lib/galleryFilters";

export default function ProjectImagePanel({
  projectId,
  results,
  total,
  page,
  pageSize,
  onSearch,
}: {
  projectId: string;
  results: GenerationResult[];
  total: number;
  page: number;
  pageSize: number;
  onSearch: (query: string, filters: GenerationSearchFilters, page: number) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<GenerationSearchFilters>({});

  const config = useMemo(
    () => ({
      ...createGallerySearchConfig(t, filters, (key, value) =>
        setFilters((current) => updateFilterValue(current, key, value)),
      ),
      title: t("projects.imagesTitle"),
    }),
    [filters, t],
  );

  return (
    <div className="rounded-[18px] border border-border-subtle bg-surface">
      <GallerySearchBar
        config={config}
        total={total}
        query={query}
        hasActiveFilters={isFilterActive(filters, query)}
        onQueryChange={setQuery}
        onSearch={() => void onSearch(query, compactFilters(filters), 1)}
        onReset={() => {
          setQuery("");
          setFilters({});
          void onSearch("", {}, 1);
        }}
      />
      <div className="p-5">
        {results.length === 0 ? (
          <EmptyCollectionState
            title={t("projects.imagesEmptyTitle")}
            subtitle={t("projects.imagesEmptyHint")}
          />
        ) : (
          <GenerationGrid results={results} favoriteMode="manage" onSelect={() => {}} onManageFolders={() => {}} />
        )}
        <PaginationControls
          page={page}
          totalPages={Math.max(1, Math.ceil(total / pageSize))}
          onPageChange={(nextPage) => void onSearch(query, compactFilters(filters), nextPage)}
        />
      </div>
    </div>
  );
}
```

Create `src/pages/ProjectHomePage.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { archiveProject, getConversations, getProjects, renameProject, searchGenerations } from "../lib/api";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { Conversation, GenerationResult, GenerationSearchFilters, Project } from "../types";
import ProjectSummaryCards from "../components/projects/ProjectSummaryCards";
import ProjectImagePanel from "../components/projects/ProjectImagePanel";

export default function ProjectHomePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { projectId = "" } = useParams();
  const { setActiveConversationId, setActiveProjectId } = useLayoutContext();
  const [project, setProject] = useState<Project | null>(null);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [showActions, setShowActions] = useState(false);

  useEffect(() => {
    setActiveProjectId(projectId);
    setActiveConversationId(null);
  }, [projectId, setActiveConversationId, setActiveProjectId]);

  useEffect(() => {
    getProjects(false).then((items) => {
      setProject(items.find((item) => item.id === projectId && item.id !== "default") ?? null);
    }).catch(() => {
      setProject(null);
    });
    getConversations(undefined, projectId, false).then(setConversations).catch(() => {
      setConversations([]);
    });
    searchGenerations(undefined, 1, false, {}, projectId).then((result) => {
      setResults(result.generations);
      setTotal(result.total);
      setPage(result.page);
      setPageSize(result.page_size);
    }).catch(() => {
      setResults([]);
      setTotal(0);
    });
  }, [projectId]);

  const recentModels = useMemo(
    () => Array.from(new Set(results.map((result) => result.generation.engine))).slice(0, 2),
    [results],
  );

  async function handleProjectAction(action: "rename" | "archive") {
    if (!project) return;

    if (action === "rename") {
      const name = window.prompt(t("sidebar.renameProject"), project.name);
      if (!name?.trim() || name.trim() === project.name) return;
      await renameProject(project.id, name.trim());
      const items = await getProjects(false);
      setProject(items.find((item) => item.id === project.id) ?? project);
      setShowActions(false);
      return;
    }

    await archiveProject(project.id);
    setShowActions(false);
    navigate("/projects");
  }

  if (!project) {
    return <div className="p-8 text-[14px] text-muted">{t("projects.notFound")}</div>;
  }

  return (
    <div className="h-full overflow-y-auto px-8 py-8">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="text-[11px] uppercase tracking-[0.08em] text-muted">{t("projects.directory")}</div>
          <h1 className="mt-2 text-[30px] font-semibold text-foreground">{project.name}</h1>
        </div>
        <div className="relative flex items-center gap-2">
          <button
            onClick={() => setShowActions((current) => !current)}
            aria-label={t("projects.manage")}
            className="rounded-[12px] border border-border-subtle px-4 py-2 text-[12px] font-medium text-foreground"
          >
            {t("projects.manage")}
          </button>
          {showActions ? (
            <div className="absolute right-0 top-[44px] z-10 w-40 overflow-hidden rounded-[10px] border border-border-subtle bg-surface py-1 shadow-card">
              <button className="w-full px-3 py-2 text-left text-[12px]" onClick={() => void handleProjectAction("rename")}>
                {t("sidebar.rename")}
              </button>
              <button className="w-full px-3 py-2 text-left text-[12px]" onClick={() => void handleProjectAction("archive")}>
                {t("sidebar.archive")}
              </button>
            </div>
          ) : null}
          <button
            onClick={() => navigate("/generate")}
            className="rounded-[12px] bg-primary px-4 py-2 text-[12px] font-medium text-white"
          >
            {t("projects.newConversation")}
          </button>
        </div>
      </div>

      <div className="mt-6">
        <ProjectSummaryCards project={project} recentModels={recentModels} />
      </div>

      <section className="mt-8">
        <h2 className="text-[18px] font-semibold text-foreground">{t("projects.recentConversations")}</h2>
        <div className="mt-4 grid gap-3">
          {conversations.length === 0 ? (
            <div className="rounded-[14px] border border-border-subtle bg-surface p-4 text-[13px] text-muted">
              {t("projects.emptyConversations")}
            </div>
          ) : (
            conversations.slice(0, 6).map((conversation) => (
              <button
                key={conversation.id}
                onClick={() => {
                  setActiveProjectId(project.id);
                  setActiveConversationId(conversation.id);
                  navigate("/generate");
                }}
                className="rounded-[14px] border border-border-subtle bg-surface p-4 text-left"
              >
                <div className="text-[14px] font-medium text-foreground">{conversation.title}</div>
                <div className="mt-1 text-[12px] text-muted">{conversation.generation_count} images</div>
              </button>
            ))
          )}
        </div>
      </section>

      <section className="mt-8">
        <ProjectImagePanel
          projectId={project.id}
          results={results}
          total={total}
          page={page}
          pageSize={pageSize}
          onSearch={async (query, filters, nextPage) => {
            const result = await searchGenerations(query || undefined, nextPage, false, filters, project.id);
            setResults(result.generations);
            setTotal(result.total);
            setPage(result.page);
            setPageSize(result.page_size);
          }}
        />
      </section>
    </div>
  );
}
```

Add copy:

```json
"projects.recentConversations": "Recent Conversations",
"projects.newConversation": "New Conversation",
"projects.manage": "Manage",
"projects.imagesTitle": "Project Images",
"projects.imagesEmptyTitle": "No project images yet",
"projects.imagesEmptyHint": "Generated images from this project will appear here.",
"projects.emptyConversations": "Start the first conversation in this project.",
"projects.notFound": "This project could not be found."
```

```json
"projects.recentConversations": "最近会话",
"projects.newConversation": "新建对话",
"projects.manage": "管理项目",
"projects.imagesTitle": "项目图片",
"projects.imagesEmptyTitle": "项目内还没有图片",
"projects.imagesEmptyHint": "这个项目生成的图片会显示在这里。",
"projects.emptyConversations": "从这个项目开始第一条会话。",
"projects.notFound": "未找到这个项目。"
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
npm test -- src/pages/ProjectHomePage.test.tsx
```

Expected: PASS, with the page loading a real project, searching generations with `projectId`, and rendering recent-conversation and image sections.

- [ ] **Step 5: Commit**

```bash
git add src/components/projects/ProjectSummaryCards.tsx src/components/projects/ProjectImagePanel.tsx src/pages/ProjectHomePage.tsx src/pages/ProjectHomePage.test.tsx src/locales/en.json src/locales/zh-CN.json
git commit -m "feat: add project home page"
```

### Task 6: Strip visible project UI out of the conversation sidebar and keep project context in Generate

**Files:**
- Modify: `src/components/sidebar/ConversationList.tsx`
- Create: `src/components/sidebar/ConversationList.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `src/components/sidebar/ConversationList.test.tsx`:

```tsx
import { MemoryRouter } from "react-router-dom";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ConversationList from "./ConversationList";

const getConversations = vi.fn();
const getProjects = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("../../lib/api", () => ({
  getConversations: (...args: unknown[]) => getConversations(...args),
  getProjects: (...args: unknown[]) => getProjects(...args),
  archiveConversation: vi.fn(),
  createProject: vi.fn(),
  deleteConversation: vi.fn(),
  deleteProject: vi.fn(),
  moveConversationToProject: vi.fn(),
  pinConversation: vi.fn(),
  pinProject: vi.fn(),
  renameConversation: vi.fn(),
  renameProject: vi.fn(),
  toAssetUrl: (path: string) => path,
  unarchiveConversation: vi.fn(),
  unarchiveProject: vi.fn(),
  unpinConversation: vi.fn(),
  unpinProject: vi.fn(),
}));

describe("ConversationList", () => {
  beforeEach(() => {
    getProjects.mockReset();
    getConversations.mockReset();

    getProjects.mockResolvedValue([
      {
        id: "project-1",
        name: "Brand Storyboards",
        created_at: "",
        updated_at: "",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        conversation_count: 12,
        image_count: 86,
      },
    ]);
    getConversations.mockResolvedValue([
      {
        id: "conversation-1",
        project_id: "project-1",
        title: "Homepage hero direction",
        created_at: "",
        updated_at: "",
        archived_at: null,
        pinned_at: null,
        deleted_at: null,
        generation_count: 8,
        latest_generation_at: "",
        latest_thumbnail: null,
      },
    ]);
  });

  it("shows project context but no visible project strip when scoped", async () => {
    render(
      <MemoryRouter>
        <ConversationList
          activeProjectId="project-1"
          activeConversationId={null}
          refreshKey={0}
          onSelectProject={vi.fn()}
          onProjectCreated={vi.fn()}
          onSelectConversation={vi.fn()}
          onInitialConversation={vi.fn()}
          onNewConversation={vi.fn()}
        />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(getConversations).toHaveBeenCalledWith(undefined, "project-1", false);
    });

    expect(await screen.findByText("Brand Storyboards")).toBeInTheDocument();
    expect(screen.queryByText("sidebar.projects")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "sidebar.newProject" })).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
npm test -- src/components/sidebar/ConversationList.test.tsx
```

Expected: FAIL because `ConversationList` still renders the projects strip and the new-project button in its header.

- [ ] **Step 3: Write the minimal implementation**

Update `src/components/sidebar/ConversationList.tsx` by removing the always-visible project strip and group headers. Keep `getProjects` for move targets and current-project lookup, then render a compact scoped header when `activeProjectId` is present:

```tsx
import { useNavigate } from "react-router-dom";

export default function ConversationList(...) {
  const navigate = useNavigate();
  // keep existing project and conversation loading

  return (
    <div className="flex h-full flex-col">
      <div className="px-4 pt-5 pb-3">
        <div className="mb-3 flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <MessageSquare size={13} className="text-muted" strokeWidth={1.8} />
            <span className="text-[13px] font-semibold text-foreground tracking-tight">
              {activeProjectId && selectedProject
                ? selectedProject.name
                : t("sidebar.conversations")}
            </span>
          </div>
          {activeProjectId && selectedProject ? (
            <button
              onClick={() => navigate(`/projects/${activeProjectId}`)}
              className="text-[11px] font-medium text-primary"
            >
              {t("projects.backToProject")}
            </button>
          ) : null}
        </div>
        <div className="relative">
          <Search size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" strokeWidth={2} />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("sidebar.search")}
            className="h-[28px] w-full rounded-[8px] border border-border-subtle bg-subtle/50 pl-7 pr-2 text-[12px] text-foreground placeholder:text-muted/60 focus:outline-none focus:border-border focus:bg-surface transition-colors"
          />
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-2.5 pb-4 pt-3">
        <button
          onClick={onNewConversation}
          className="mb-3 flex w-full items-center gap-2.5 rounded-[10px] border border-dashed border-border-subtle px-2 py-2 text-left transition-all hover:border-primary/30 hover:bg-primary/4"
        >
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[8px] border border-border-subtle bg-subtle">
            <Plus size={14} className="text-primary" strokeWidth={2} />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-[12px] leading-snug text-foreground">
              {t("sidebar.newConversation")}
            </p>
            <div className="mt-0.5 flex items-center gap-1.5">
              <span className="truncate text-[10px] text-muted/60">
                {selectedProject?.name ?? t("sidebar.conversations")}
              </span>
            </div>
          </div>
        </button>

        <div className="flex flex-col gap-0.5">
          {conversations.map((conv, i) => (
            <ConversationRow
              key={conv.id}
              conversation={conv}
              active={activeConversationId === conv.id}
              index={i}
              menuOpen={openMenu?.type === "conversation" && openMenu.id === conv.id}
              onSelect={() => onSelectConversation(conv.id)}
              onToggleMenu={() =>
                setOpenMenu((current) =>
                  current?.type === "conversation" && current.id === conv.id
                    ? null
                    : { type: "conversation", id: conv.id },
                )
              }
              onAction={(action) => void runConversationAction(conv, action)}
              projects={projects.filter((project) => project.id !== "default")}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
```

Add the last missing copy:

```json
"projects.backToProject": "Back to project"
```

```json
"projects.backToProject": "返回项目"
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
npm test -- src/components/sidebar/ConversationList.test.tsx
```

Expected: PASS, with the sidebar showing conversation-only UI plus a scoped project header and back link when `activeProjectId` is set.

- [ ] **Step 5: Run the full verification pass**

Run:

```bash
npm test
npm run build
cargo test generation_filters_to_sql_adds_project_scope_clause
cargo check
```

Expected:

- all Vitest suites PASS
- Vite build completes successfully
- the new Rust gallery test PASSes
- `cargo check` exits 0

- [ ] **Step 6: Commit**

```bash
git add src/components/sidebar/ConversationList.tsx src/components/sidebar/ConversationList.test.tsx src/locales/en.json src/locales/zh-CN.json
git commit -m "refactor: separate project and conversation navigation"
```
