---
type: meta
title: "Dashboard"
updated: 2026-05-09
tags: [meta, dashboard]
---

# Wiki Dashboard

## Recent Activity
```dataview
TABLE type, status, updated FROM "wiki" SORT updated DESC LIMIT 15
```

## Architecture Pages
```dataview
LIST FROM "wiki/architecture" SORT title ASC
```

## Flow Pages
```dataview
LIST FROM "wiki/flows" SORT title ASC
```

## Module Pages
```dataview
LIST FROM "wiki/modules" SORT title ASC
```

## History Pages
```dataview
LIST FROM "wiki/history" SORT title ASC
```
