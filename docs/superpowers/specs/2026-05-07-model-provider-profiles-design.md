# Model Provider Profiles Design

## Goal

让每个绘图模型支持多个用户自定义服务商配置。用户可以给同一个模型配置多套 API Key 和 endpoint，例如给 `gpt-image-2` 配置 OpenAI 官方、公司代理、备用网关等，并选择当前启用哪一套。

第一版重点是连接配置，不改变模型目录、生成参数、历史记录筛选和图片保存流程。

## Current State

Astro Studio 现在已经支持按模型保存连接配置：

- 前端模型目录在 `src/lib/modelCatalog.ts`
- 设置页模型配置在 `src/pages/SettingsPage.tsx` 和 `src/components/settings/ModelSettingsPanel.tsx`
- Tauri IPC 包装在 `src/lib/api.ts`
- 后端设置读取和保存集中在 `src-tauri/src/lib.rs`
- endpoint 默认值和模型规范化在 `src-tauri/src/model_registry.rs`
- 持久化使用 `settings` 表的 key/value 结构

当前持久化形态是每个模型一套配置：

- `model_config::<model>::api_key`
- `model_config::<model>::endpoint_mode`
- `model_config::<model>::base_url`
- `model_config::<model>::generation_url`
- `model_config::<model>::edit_url`

生成和编辑时，后端通过模型名读取这套配置，再调用 OpenAI 或 Gemini 对应的 image engine。

## Chosen Approach

使用现有 `settings` 表保存每个模型的 provider profiles JSON，并为每个模型保存一个 active provider id。

新增设置 key：

- `model_provider_profiles::<model>`：该模型的服务商配置数组 JSON
- `model_active_provider::<model>`：该模型当前启用的 profile id

这是推荐方案，因为它：

- 不需要新建数据库表或复杂迁移
- 与现有 key/value settings 模型一致
- 可以自然兼容旧的单套模型配置
- 对现有生成历史里的 `engine` 字段零侵入
- 后续如果要迁移到独立 provider 表，也有清晰边界

不采用“把服务商当成模型变体”的方案，因为那会污染画廊模型筛选、历史记录和共享模型目录。

## Product Rules

- 服务商配置完全由用户自定义命名，第一版不内置模板列表。
- 服务商配置从属于一个具体模型，不跨模型共享。
- 每个模型必须始终有且只有一个 active provider。
- 用户可以新增、重命名、编辑、删除和启用 provider profile。
- 如果某个模型只有一个 provider，不能删除到空状态。
- 旧版单套配置自动显示为该模型的默认 provider。
- 生成页默认使用当前模型的 active provider，不在第一版增加生成页临时 provider 选择器。
- 历史记录继续按模型记录，不额外记录 provider 名称，避免把连接细节暴露到作品浏览流程。

## Data Model

前后端共享一个逻辑模型：

```ts
interface ModelProviderProfile {
  id: string;
  name: string;
  api_key: string;
  endpoint_settings: EndpointSettings;
}

interface ModelProviderProfilesState {
  active_provider_id: string;
  profiles: ModelProviderProfile[];
}
```

`id` 使用 UUID 或稳定随机 id，由后端创建。`name` 是用户可编辑展示名。`api_key` 沿用现有 API Key 处理规则，保存前移除多余空白和 `Bearer` 前缀。`endpoint_settings` 沿用现有 `EndpointSettings`：

- `mode`
- `base_url`
- `generation_url`
- `edit_url`

默认 provider 名称使用 `Default`。从旧配置读出的默认 provider id 使用稳定 id，例如 `default`，这样 active provider 迁移和前端测试更可预测。

## Backend Design

### 1. Provider Profile Helpers

在 `src-tauri/src/models.rs` 增加 provider profile 相关结构体和设置 key 常量。

在 `src-tauri/src/lib.rs` 增加 helper：

- `model_provider_profiles_key(model)`
- `model_active_provider_key(model)`
- `default_provider_profile_for_model(db, model)`
- `read_model_provider_profiles_state(db, model)`
- `save_model_provider_profiles_state(db, model, state)`
- `active_provider_profile_for_model(db, model)`

读取逻辑：

1. 规范化模型名。
2. 尝试读取 `model_provider_profiles::<model>` JSON。
3. 如果没有 JSON 或 JSON 无效，按旧的 `model_config::<model>::...` 读取默认 provider。
4. 确保 profiles 非空。
5. 读取 active provider id；如果为空或不存在，回退到第一项。

保存逻辑：

1. 规范化模型名。
2. 规范化每个 profile 的名称、API Key 和 endpoint。
3. 去除空 id 或重复 id，必要时返回错误。
4. 确保 active provider 存在。
5. 写入 profiles JSON 和 active provider id。

### 2. IPC Commands

新增命令：

- `get_model_provider_profiles(model) -> ModelProviderProfilesState`
- `save_model_provider_profiles(model, active_provider_id, profiles)`
- `create_model_provider_profile(model, name) -> ModelProviderProfilesState`
- `delete_model_provider_profile(model, provider_id) -> ModelProviderProfilesState`
- `set_active_model_provider(model, provider_id) -> ModelProviderProfilesState`

第一版可以只让前端调用“读取整个 state”和“保存整个 state”，但后端仍应保留小粒度 helper，方便测试和后续扩展。

兼容命令继续保留：

- `get_model_api_key`
- `save_model_api_key`
- `get_model_endpoint_settings`
- `save_model_endpoint_settings`

这些命令改为读写该模型 active provider，保证旧前端调用和现有测试语义不破坏。

### 3. Generation Resolution

`generate_image` 和 `edit_image` 当前通过模型读取 API Key 和 endpoint。新逻辑改为：

1. 规范化请求模型。
2. 读取该模型 active provider。
3. 从 active provider 取 API Key。
4. 从 active provider endpoint settings 解析 generation 或 edit URL。
5. 后续请求体、保存、事件和日志流程保持不变。

错误文案保持简单：

- 没有 API Key：`API key not set for the active provider. Please set it in Settings.`
- active provider 不存在：自动回退到第一个 provider，不直接打断生成。
- profiles JSON 损坏：回退到旧配置生成默认 provider，并写 runtime warning。

## Frontend Design

### 1. API Layer

在 `src/types/index.ts` 增加 provider profile 类型。

在 `src/lib/api.ts` 增加：

- `getModelProviderProfiles(model)`
- `saveModelProviderProfiles(model, state)`
- `createModelProviderProfile(model, name)`
- `deleteModelProviderProfile(model, providerId)`
- `setActiveModelProvider(model, providerId)`

现有 `getModelApiKey` / `saveModelApiKey` / `getModelEndpointSettings` / `saveModelEndpointSettings` 可以继续存在，但设置页会逐步转向 provider profiles API。

### 2. Settings State

`SettingsPage` 的模型 tab 由“当前模型的一套 key/url 状态”升级为“当前模型的 provider state”：

- `providerState`
- `activeProviderId`
- `selectedProviderId`
- `providerSaved`
- `providerLoading`

当用户切换模型：

1. 先重置为该模型默认 provider 的占位状态。
2. 请求该模型 provider profiles。
3. 如果用户已经切到其他模型，忽略迟到响应。
4. 将 selected provider 设置为 active provider。

现有防止迟到请求覆盖当前模型的逻辑保留，并扩展到 provider state。

### 3. Model Settings Panel UI

模型选择卡片保留。

在模型卡片下方增加“服务商配置”区域：

- 左侧或上方：provider profiles 列表
- 每个 item 显示名称、是否 active、endpoint 类型、小型状态
- 操作：选择、启用、删除
- 新增按钮：创建一个名为 `New Provider` 的 provider

右侧或下方显示 selected provider 的编辑表单：

- provider name
- API Key
- endpoint mode
- base URL 或 full URLs
- 保存按钮

删除规则：

- 如果只剩一个 provider，删除按钮禁用。
- 如果删除 active provider，保存后自动启用剩余列表第一项。
- 如果删除 selected provider，选择新的 active provider 或第一项。

第一版不加 provider 模板、不做导入导出、不做连接测试按钮。

### 4. UI Draft

设置页保持现有三段式节奏：顶部是模型选择卡片，下面是当前模型的服务商配置管理区。服务商配置区使用一个单层面板，不在卡片里再套卡片。

桌面布局：

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Model Configuration                                                          │
│ Connection settings for image generation                                     │
│                                                                              │
│ ┌────────────────────────┐ ┌────────────────────────┐ ┌───────────────────┐ │
│ │ GPT Image 2            │ │ Nano Banana            │ │ Nano Banana 2     │ │
│ │ OPENAI                 │ │ GOOGLE                 │ │ GOOGLE            │ │
│ │ gpt-image-2            │ │ gemini-2.5-flash-image │ │ gemini-3.1...     │ │
│ │ Edit image  Separate   │ │ Edit image  Shared     │ │ Edit image Shared │ │
│ └────────────────────────┘ └────────────────────────┘ └───────────────────┘ │
│                                                                              │
│ ──────────────────────────────────────────────────────────────────────────── │
│                                                                              │
│ Providers                                      [+ New Provider]              │
│                                                                              │
│ ┌───────────────────────────────┐  ┌───────────────────────────────────────┐ │
│ │ ● OpenAI Official      Active │  │ Provider Name                         │ │
│ │   Base URL                    │  │ [OpenAI Official____________________] │ │
│ │   https://api.openai.com/v1   │  │                                       │ │
│ │                               │  │ API Key                               │ │
│ │ ○ Company Gateway             │  │ [sk-...________________________][eye] │ │
│ │   Full URLs                   │  │                                       │ │
│ │   https://proxy.example/...   │  │ Endpoint                              │ │
│ │                               │  │ [ Base URL ][ Full URLs ]             │ │
│ │ ○ Backup Key                  │  │ [https://api.openai.com/v1__________] │ │
│ │   Base URL                    │  │                                       │ │
│ │   https://api.openai.com/v1   │  │                         [Save] [Use]  │ │
│ └───────────────────────────────┘  └───────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

Provider list behavior:

- Active provider 使用实心状态点、`Active` 标记和更强的边框。
- Selected provider 使用轻微底色，允许 selected 和 active 是不同项。
- 每一项显示 provider 名称、endpoint 模式、短 URL。
- item 右侧在 hover 或 focus 时显示 `Use` 和删除图标。
- 只剩一个 provider 时删除图标禁用。

Provider editor behavior:

- 编辑区始终显示 selected provider。
- `Use` 按钮只在 selected provider 不是 active provider 时可用。
- `Save` 保存名称、API Key 和 endpoint。
- 保存成功后按钮短暂显示 `Saved`。
- API Key 复用现有眼睛图标显示/隐藏模式。
- endpoint mode 继续使用现有 segmented control。

窄屏布局：

```text
┌──────────────────────────────┐
│ Model Configuration           │
│                               │
│ [ GPT Image 2              ✓ ]│
│ [ Nano Banana                ]│
│ [ Nano Banana 2              ]│
│                               │
│ Providers        [+]          │
│ [● OpenAI Official   Active ] │
│ [○ Company Gateway          ] │
│ [○ Backup Key               ] │
│                               │
│ Provider Name                 │
│ [OpenAI Official___________]  │
│ API Key                       │
│ [sk-..._______________][eye] │
│ Endpoint                      │
│ [ Base URL ][ Full URLs ]     │
│ [https://api.openai.com/v1_]  │
│                               │
│                 [Save] [Use]  │
└──────────────────────────────┘
```

Narrow screens stack provider list above the editor. The model cards use the existing two-column-to-one-column responsive behavior. Buttons keep fixed heights so saved states and labels do not shift the layout.

Empty and first-run states:

- A model with no saved provider JSON shows one `Default` provider built from legacy settings.
- A newly created provider is selected immediately, named `New Provider`, and uses the selected model's default endpoint settings.
- If API Key is empty, the provider can still be saved; generation later shows the missing-key error.

Visual style:

- Reuse the current warm neutral surface, `border-border-subtle`, `bg-subtle/20`, `shadow-card`, and primary accent.
- Provider items should be rows, not large marketing cards, because this is an operational settings surface.
- Use lucide icons: `Plus` for new provider, `Trash2` for delete, `Check` for active/saved, `Eye/EyeOff` for key visibility.
- Keep card radii at the existing 8-12px range and avoid decorative backgrounds.

### 5. Copy And Localization

UI copy 继续使用英文 key，并补齐多语言文件。新增 key 示例：

- `settings.providers`
- `settings.providersDesc`
- `settings.providerName`
- `settings.newProvider`
- `settings.activateProvider`
- `settings.activeProvider`
- `settings.deleteProvider`
- `settings.saveProvider`

中文翻译可使用“服务商配置”“启用”“当前启用”等表达。

## Compatibility

旧设置迁移采用 lazy migration，不在数据库启动时强制写入：

- 当读取某模型 provider profiles 且新 JSON 不存在时，后端从旧 keys 构建 `Default` provider。
- 当用户保存 provider profiles 后，写入新 JSON。
- 旧 keys 不立即删除。
- GPT Image 2 继续同步旧的 `api_key` / `base_url` 等全局 legacy keys，降低回退风险。

这样用户升级后不需要重新配置 key，也不会因为迁移失败导致生成不可用。

## Error Handling

- provider name 为空时保存为 `Provider` 或拒绝保存；实现时优先选择前端禁用保存并给输入框保持焦点。
- API Key 可以为空保存，但生成时会报缺少 key；这允许用户先建 provider 再补 key。
- endpoint 为空时填充所选模型默认值。
- Gemini 模型继续强制 shared generation/edit endpoint。
- JSON 解析失败不阻塞应用，回退默认 provider 并写日志。
- 删除 active provider 后必须重新选择一个 active provider。

## Testing

### Frontend

新增或扩展 Vitest：

- provider profiles IPC wrapper 参数正确。
- 设置页切换模型时加载该模型 provider profiles。
- 新增 provider 后显示新配置并可编辑保存。
- 启用 provider 后保存 active provider id。
- 删除 active provider 后自动选择剩余 provider。
- 迟到的 provider profile 响应不会覆盖当前模型。
- 旧的 key/url UI 行为由 active provider 承载。

### Backend

新增或扩展 Rust tests：

- 无 provider JSON 时从旧模型配置构建默认 provider。
- active provider id 缺失或无效时回退第一项。
- 保存 provider profiles 会规范化 key、endpoint 和 active id。
- 兼容命令读写 active provider。
- `generate_image` / `edit_image` 的 provider resolution helper 使用 active provider 的 API Key 和 endpoint。
- Gemini provider profile 的 edit URL 跟随 generation URL。

### Verification

实现完成后运行：

```bash
npm test
npm run build
cargo test
cargo build
```

如果 UI 结构变化较大，再启动 Vite 或 Tauri dev server 做一次设置页手动检查。

## Out Of Scope

- 预置服务商模板。
- 按项目覆盖 provider。
- 在生成页临时选择 provider。
- provider 连通性测试。
- provider 级使用统计和失败率。
- 把 provider 名称写入生成历史。
