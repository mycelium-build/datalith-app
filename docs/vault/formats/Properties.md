---
category: format
---

# Properties

Properties are structured data attached to a file. A Markdown file may start with an optional block of properties, written as a **YAML frontmatter**: a mapping between two `---` lines at the very top of the file.

```yaml
---
category: guide
status: active
tags:
  - docs
  - example
priority: 3
published: true
---
```

The frontmatter is a plain YAML **mapping**, a list of `key: value` pairs. Datalith exposes these as the file's properties.

## Supported values

- **Text:** `category: guide`
- **Numbers:** `priority: 3`
- **Booleans:** `published: true`
- **Lists:** `tags: [docs, example]`
- **Links:** a value like `[[Welcome]]` is recognized as a wiki link and displayed as a property link.

A file that does not start with `---` simply has no properties. Malformed YAML is reported rather than silently dropped, and the rest of the file still opens.

## What properties are used for

Properties are data, not decoration: they power data-driven views. Graph filters and groups reference them by name, e.g. `category == "guide"`, to select or color nodes. See [[Graph]] and [[Overview.graph]] in this Vault.
