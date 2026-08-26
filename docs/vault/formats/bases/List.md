---
category: format
---

A **List View** renders each matching file as one compact row. Declare it with `type: list`:

```yaml
views:
  - type: list
    name: Reading list
    order:
      - file.name
      - author
    markers: bullets
    indentProperties: true
    separators: ", "
```

# List Settings

- `markers`: `bullets`, `numbers`, or `none`.
- `indentProperties`: display the remaining properties below the primary item.
- `separators`: text between inline properties.

Shared syntax — filters, ordering, grouping, summaries, and limits — is documented in [[formats/bases/Overview|the Base overview]].
