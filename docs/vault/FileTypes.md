---
category: reference
---

Every file Datalith can open has a registered **File Type**. Each type declares which capabilities its files support:

| File Type | Editor | Viewer | Text search | Wiki Links | Properties |
| --- | --- | --- | --- | --- | --- |
| `.md` Note | ✓ | ✓ | ✓ | ✓ | ✓ |
| `.graph` Graph | ✓ | ✓ | — | — | — |
| `.todotxt` To-do | ✓ | — | ✓ | — | — |
| Images | — | ✓ | — | — | — |

- **Editor**: files open in a dedicated editor, not just a viewer.
- **Viewer**: files get a rendered preview (Markdown preview or the Graph View).
- **Text search**: file names and contents are indexed and searchable.
- **Wiki Links**: `[[links]]` are resolved and feed the graph edges.
- **Properties**: leading YAML frontmatter is read as structured data. See [[Properties]].

Anything else in a Vault is simply browsed in the file tree and never indexed.
