---
status: accepted
---

# Use a queryable catalog database as the Vault read model

Datalith needs several derived views of a Vault: Graph Views, full-text search, the palette and quick switcher, Wiki Link resolution, and future database-like views of file properties such as [Obsidian Bases](https://obsidian.md/help/bases). Building each feature directly from the filesystem made every consumer responsible for discovering files and could repeatedly read and parse the same files. Graph construction, for example, walked every catalogued Markdown path and reread its frontmatter before it could apply a filter.

We use an embedded Turso database at `.datalith/catalog.db` as a queryable, disposable read model maintained by the Vault Catalog. For every Tracked File it projects the Vault-relative path, extension, folder, size, modification time, typed YAML Frontmatter, and authored and resolved Wiki Links. A typed `CatalogQuery` is compiled into parameterized database filters over file fields and nested metadata, and related documents and links can be read from one consistent snapshot. Full-text content remains in the specialized Search Index, but that index is built and incrementally updated from catalogued paths instead of walking the Vault independently.

The Vault filesystem remains the only source of truth. Opening a Vault reconciles the database with registered File Types, and filesystem notifications update the database, Search Index, and subscribers together. Schema versions may drop and rebuild the database rather than migrate authoritative user data. Catalog initialization and graph queries run away from the UI thread so the Vault can remain responsive while derived state is prepared.

## Considered options

- **Let each feature scan and parse the Vault on demand:** keeps storage simple, but duplicates discovery and parsing work, makes latency proportional to Vault size for every refresh, and lets consumers disagree about which files and metadata are current.
- **Keep separate in-memory path, metadata, and Wiki Link indexes:** avoids an embedded database, but requires custom query and synchronization machinery, consumes memory for the whole Vault, and makes compound property filters and consistent document/link reads harder.
- **Treat the catalog database as authoritative storage:** would make queries straightforward, but would conflict with Datalith's local-file model and with files changed by users or other applications.

## Consequences

- Metadata-driven features query one shared projection rather than rereading every candidate file. This is the foundation for fast Graph Views, Bases-like tables and cards, property-aware palettes, and other app elements.
- New file types declare whether they support text search, Wiki Links, and YAML Frontmatter; the catalog only reads and projects the capabilities a type needs.
- Filesystem reconciliation is the architectural boundary for keeping all derived views current. Consumers subscribe to catalog events instead of maintaining their own filesystem view.
- The database can be temporarily unavailable or behind the filesystem during initialization or reconciliation, so UI consumers must tolerate loading and refresh states.
- Datalith now carries an embedded database dependency and must maintain schema, query compilation, transactional updates, path normalization, and filesystem-event reconciliation.
- Catalog queries accelerate selection by avoiding repeated I/O and parsing; they do not remove feature-specific work such as graph layout, full-text ranking, or rendering.
