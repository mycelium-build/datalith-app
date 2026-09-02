---
category: format
---

A **Table View** renders matching files as rows with one column per property. Declare it with `type: table`:

```yaml
views:
  - type: table
    name: Recently updated
    order:
      - file.name
      - status
      - file.mtime
    sort:
      - property: file.mtime
        direction: DESC
    rowHeight: medium
```

# Table Settings

`rowHeight` accepts `short`, `medium`, `tall`, and `extra tall`.

Shared syntax — filters, ordering, grouping, summaries, and limits — is documented in [[formats/bases/Overview|the Base overview]].
