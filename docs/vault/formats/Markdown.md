---
category: format
---

# Markdown

Datalith edits Markdown files with a live preview and renders them with standard Markdown plus a couple of additions.

## Core syntax

- **Headings:** `#` to `######`
- **Emphasis:** `*italic*`, `**bold**`, `~~strikethrough~~`
- **Links:** `[label](https://example.com)` or `[[https://example.com]]`, external links open in your browser
- **Images:** `![alt](image.png)`
- **Lists:** ordered and unordered, with nesting
- **Code:** inline `` `code` `` and fenced blocks with ``` ```lang ```
- **Quotes:** `> blockquote`
- **Rules:** `---`, `***`, `___`

## Links

There are two kinds of links:

- `[[text]]` — a wiki link to another file in the Vault, for example `[[Welcome]]` or `[[formats/Graph]]`.
- `[text](target)` — a normal link; external links open in your browser.

Wiki links can add a label: `[[Welcome|home]]`. A name-only link resolves to the unique same-folder target; a path link is exact. Use **⌘↩** on a link to jump.

## Properties

A file may start with [[Properties|properties]], written as YAML frontmatter:

```yaml
---
category: format
---
```

Properties are used for data-driven views: the [[Graph|Graph View]] selects and colors nodes by them. See [[Properties]] for the details.
