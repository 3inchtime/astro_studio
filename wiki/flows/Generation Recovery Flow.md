---
type: concept
title: "Generation Recovery Flow"
created: 2026-05-09
updated: 2026-05-09
tags: [flows, recovery, reliability]
status: active
---

# Generation Recovery Flow

## 问题场景

当 Astro Studio 在图像生成过程中崩溃或被关闭：
1. API 请求已发送，响应已收到
2. 但图片数据未保存到文件系统
3. 数据库中 Generation 记录仍为 `status=processing`

## 恢复机制

### 写入阶段

在生成过程中，引擎会将 API 响应体写入文件：

```
API 响应成功
    ↓
write_response_body() → logs/responses/YYYYMMDD_HHMMSS_mmm.json
    ↓
插入 generation_recoveries 记录
    ├─ request_state = "response_ready"
    ├─ response_file = "path/to/response.json"
    ├─ output_format = "png"
```

### 恢复阶段

应用启动时，在 `lib.rs` 的 `setup` 阶段：

```
recover_interrupted_generations()
    ↓
查询 status=processing 的 Generation
    ↓
对每个待恢复记录:
    ├─ request_state == "response_ready"?
    │   ├─ 读取 response_file
    │   ├─ engine.decode_images_from_response()
    │   ├─ save_generation_images_for_recovery()
    │   │   ├─ 保存图片到文件系统
    │   │   ├─ 更新 status=completed
    │   │   └─ 删除 recovery 记录
    │   └─ 日志记录恢复结果
    ├─ request_state 为空?
    │   └─ 标记为 failed (中断消息)
    └─ response_file 缺失?
        └─ 标记为 failed
```

### 清理阶段

启动时还会清理过期的 recovery 记录：

```rust
// 清理非 processing 状态的 recovery
DELETE FROM generation_recoveries
WHERE generation_id IN (
    SELECT id FROM generations WHERE status != 'processing'
)
```

## 恢复状态

| request_state | 说明 | 处理 |
|---------------|------|------|
| `response_ready` | API 响应已保存 | 解码图片并保存 |
| 无 | 请求已发送但无响应 | 标记为 failed |

## 数据流

```
┌─────────────────────────────────────────────┐
│              正常生成流程                      │
│  API 请求 → 响应 → 解码 → 保存图片 → 完成     │
└─────────────────────────────────────────────┘
                    ↓ 崩溃点
┌─────────────────────────────────────────────┐
│              恢复流程                          │
│  启动 → 查询 processing → 读取响应文件        │
│  → 解码图片 → 保存图片 → 标记完成              │
└─────────────────────────────────────────────┘
```

## 关键代码路径

- 恢复逻辑：`src-tauri/src/lib.rs` → `recover_interrupted_generations`
- 响应保存：`src-tauri/src/api_gateway.rs` → `write_response_body`
- Recovery 表：`db.rs` migration v10
- 图片保存：`lib.rs` → `save_generation_images_for_recovery`

## 相关页面

- [[Image Generation Flow]] - 图像生成流程
- [[Database Schema]] - 数据库设计
