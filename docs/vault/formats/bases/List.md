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

# Summaries

When the view declares `summaries`, a `Summary` entry is inserted at the top of the list — or right after each group header when grouped — listing one line per summary:

```text
- Summary
  - Pages Sum: 350
  - Rating roundedAverage: 4.0
- My note
```

Summary entries are never numbered and do not shift row numbers.

Shared syntax — filters, ordering, grouping, summaries, and limits — is documented in [[formats/bases/Overview|the Base overview]].
