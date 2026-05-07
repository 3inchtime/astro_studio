# Project Home and Gallery Simplification Design

## Goal

Reshape Astro Studio so that projects become a first-class destination instead of a secondary layer embedded inside the conversation sidebar, while also simplifying image filtering down to the three signals the user actually wants:

- prompt text
- model
- date

The intended product outcome is:

- `Projects` becomes a top-level app section
- each project has its own homepage
- default conversation workflow stays available, but no longer visually absorbs project management
- image search feels lighter and more understandable in both the global gallery and project pages

## Current State

The current frontend blends two concepts into one sidebar:

- conversations
- projects

This makes projects feel like a tagging mechanism attached to the default conversation workspace rather than a distinct work surface.

The current gallery search is also over-specified. It exposes many low-frequency filters such as request type, status, quality, background, format, moderation, fidelity, and source count. This creates visual noise and pushes the search UI away from the user's actual retrieval behavior.

There is already solid backend groundwork for projects:

- projects exist in the database
- conversations belong to projects
- project CRUD already exists through Tauri commands

The redesign therefore focuses primarily on frontend information architecture plus one targeted gallery-search backend extension.

## Chosen Approach

Introduce `Projects` as a top-level route with its own dedicated views:

- `/projects`
- `/projects/:projectId`

At the same time, simplify generation search everywhere to a compact three-filter model:

- prompt text input
- model selector
- date range control

This approach is preferred because it:

- gives projects a clear and independent identity
- keeps the default conversation experience intact
- avoids inventing a second generation workspace
- reuses the existing project and conversation backend model
- reduces gallery complexity without weakening core retrieval power

## Product Rules

These rules define the intended UX and should stay stable through implementation:

- projects are navigated from a top-level `Projects` entry, not from the conversation sidebar
- project management is visually separate from the default session workspace
- the default project remains a system fallback, not a peer-facing primary project in the new project UI
- every project has a homepage that combines overview, recent conversations, and project images
- image filtering is limited to prompt, model, and date
- project pages can launch new project-scoped conversations into the existing Generate workspace

## Architecture

### 1. Route Model

Add a new top-level navigation item: `Projects`.

Add these routes:

- `/projects`
- `/projects/:projectId`

Route responsibilities:

- `/generate`: active conversation workspace
- `/projects`: project directory and entry point
- `/projects/:projectId`: project homepage
- `/gallery`: app-wide image gallery

The Generate workspace stays the place where actual prompt composition and threaded creation happen. The Projects section becomes the place where project organization and project-level review happen.

### 2. Route-Aware Secondary Sidebar

The current `AppLayout` always renders the same secondary sidebar. That should become route-aware.

Behavior by section:

- on `/generate`, the secondary sidebar shows conversations only
- on `/projects` and `/projects/:projectId`, the secondary sidebar shows project navigation only
- on all other existing top-level routes, keep the current conversation-sidebar behavior for this design

This is the key structural move that keeps project UI from visually merging back into the default session shell.

### 3. Project Directory Page

`/projects` becomes a dedicated list view for projects.

The page should:

- show user-facing projects as cards or rows
- provide create-project entry points
- display lightweight metadata such as updated time, conversation count, and image count
- make one project visually prominent when recently active or pinned

The purpose of this page is not to show conversations directly. It is an index into project spaces.

### 4. Project Homepage

`/projects/:projectId` is the main deliverable of this redesign.

It contains three primary zones:

- project overview
- recent conversations
- project images

#### Project overview

This section shows:

- project name
- updated time
- active conversation count
- image count
- dominant or recent models used

It includes a primary `New Conversation` action and a secondary project-management entry point for actions such as rename and archive.

#### Recent conversations

This section shows the most recent project-scoped conversations, each with:

- title
- recent activity time
- image count or generation count
- thumbnail when available

Selecting a conversation opens the existing Generate workspace for that conversation.

#### Project images

This section shows image results only for the current project.

It reuses the existing gallery-style grid and detail interactions where possible:

- select image
- open detail
- save image
- continue editing
- manage folders

This section should feel like a project-focused gallery preview, not a completely separate media system.

### 5. Default Project Treatment

The backend default project should remain as a persistence mechanism, but the new project UX should not treat it as a normal visible peer project.

Frontend rule:

- conversations created outside explicit projects continue to use the backend default project under the hood
- the `default` project is hidden from the new `Projects` directory and project homepage navigation
- the default conversation workflow remains represented by the Generate section, not by a visible "Default Project" card

This rule is important because showing "Default Project" inside the new project destination would immediately blur the distinction the redesign is trying to establish.

### 6. Generate Workspace Relationship

The Generate page remains the creation workspace, but it must become clearly project-aware when opened from a project.

Required behavior:

- creating a new conversation from a project homepage opens Generate with that `projectId` active
- opening a recent conversation from a project homepage opens Generate with both `projectId` and `conversationId` active
- the Generate layout should show the current project context clearly when a project-scoped conversation is active

The project context indicator can be lightweight, but it must exist. Suitable examples include:

- a project chip near the conversation header
- a "Back to project" control
- a labeled conversation sidebar header

This prevents project work from feeling like it has fallen back into an unscoped global conversation mode.

## Gallery Simplification

### 1. Filter Set

Reduce generation search filters to:

- prompt text
- model
- date

Remove these from the visible UI:

- request type
- status
- size
- quality
- background
- output format
- moderation
- input fidelity
- source image count

### 2. Date Filter Shape

The user asked for a single conceptual `date` filter. Internally, that should still be backed by the existing `created_from` and `created_to` fields.

Frontend rule:

- expose date as one compact date-range control surface
- allow an unset state
- keep the backing payload as `created_from` and `created_to`

This keeps the UI simple without requiring backend schema changes.

### 3. Surfaces Using the Simplified Filters

Apply the simplified filter model in both places:

- global gallery
- project homepage image section

The global gallery continues to search across the whole app.
The project homepage image section searches only within the active project.

### 4. Search Interaction

Search behavior should stay straightforward:

- typing prompt text and pressing Enter runs search
- changing model or date updates pending filters
- explicit search action still applies the current filter state
- reset clears prompt text and date range and resets model to all

## Data Flow

### 1. Existing APIs To Reuse

Frontend should continue to reuse these existing commands where possible:

- `getProjects`
- `createProject`
- `renameProject`
- `getConversations`
- `createConversation`
- `getConversationGenerations`

### 2. Required Search Extension

The one meaningful API extension in this design is project-scoped generation search.

Extend the current generation search flow so that `searchGenerations` accepts an optional `projectId`.

Logical behavior:

- if `projectId` is absent, search across all matching generations
- if `projectId` is present, only return generations whose conversation belongs to that project

This should be implemented front to back:

- TypeScript API wrapper
- shared frontend typing
- Tauri command payload
- Rust search SQL builder

### 3. Project Homepage Loading

Project homepage data should load in parallel from three sources:

- project metadata
- recent conversations
- project images

Representative loading model:

- project metadata: derive from `getProjects()` in this iteration rather than introducing a new detail endpoint
- recent conversations: `getConversations(undefined, projectId, false)`
- project images: `searchGenerations(query, page, false, filters, projectId)`

Parallel loading keeps the page responsive and avoids serial dependency where none is required.

## Components and Boundaries

### 1. Layout Layer

Update `AppLayout` so the secondary column is route-aware instead of hardcoded to the conversation list.

This is a layout responsibility, not a page responsibility.

### 2. Project Navigation Components

Create dedicated project-facing components rather than stretching the existing conversation sidebar component further.

Representative responsibilities:

- project directory sidebar or navigator
- project list page
- project homepage header and metric cards
- recent conversation panel
- project image results panel

This keeps project UI from inheriting conversation-list assumptions.

### 3. Gallery Filter Components

Refactor current gallery filter config so it only describes:

- model
- date

Prompt text remains the main search input rather than a config-driven field.

This is a good place to shrink the config surface rather than leaving dead abstractions behind.

## Empty States and Error Handling

### 1. Project Not Found

If a project route references a missing or deleted project:

- show a dedicated not-found state inside the Projects section
- do not silently redirect to Generate
- keep the user within project navigation

### 2. Empty Project

If a project exists but has no conversations yet:

- still render the project homepage shell
- show overview data
- show a clear "start the first conversation" entry point
- keep the image section present with an empty state

### 3. Empty Image Results

If project image filters produce no matches:

- keep the filters visible
- show an empty search result state
- do not collapse the image section away

### 4. Search Failure

If project image search fails:

- keep project overview and recent conversations visible
- isolate the failure to the image section
- show a retryable inline error state

This ensures the page degrades by region rather than failing as a whole.

## Testing Strategy

Add or update tests in five groups.

### 1. Gallery filter tests

- compacted filters only preserve model and date range values
- active-filter logic works for prompt text, model, and date
- filter config only exposes the simplified fields

### 2. Global gallery page tests

- global gallery searches with prompt, model, and date
- removed filters are no longer rendered
- reset clears the simplified filter set

### 3. Project page tests

- `/projects` renders project directory content
- `/projects/:projectId` renders overview, recent conversations, and image section
- empty project states render correctly
- missing project renders a not-found state

### 4. Generate workspace context tests

- creating a conversation from a project activates the correct `projectId`
- opening a project conversation preserves project context in Generate
- project context indicator appears when relevant

### 5. Search API tests

- `searchGenerations` forwards optional `projectId`
- backend project-scoped search only returns generations belonging to that project
- unscoped search behavior remains unchanged

## Scope Boundaries

Included in this design:

- top-level Projects navigation
- project directory page
- project homepage
- route-aware secondary sidebar
- project-scoped image search
- simplified gallery filters
- Generate workspace project context indicator

Not included in this design:

- changing the database schema for projects
- rewriting conversation storage
- URL-deep-linking full conversation state into Generate
- multi-level project folders
- project permissions or collaboration roles
- changing favorite-image or trash architecture

## Implementation Notes

Two implementation decisions should be treated as deliberate constraints:

- prefer reusing the existing image grid, detail panel, and pagination patterns rather than inventing a second gallery system for projects
- prefer hiding the backend default project from project-facing UI rather than trying to explain it as a normal project

These choices keep the redesign focused and reduce the risk of turning a navigation cleanup into a wider product rewrite.
