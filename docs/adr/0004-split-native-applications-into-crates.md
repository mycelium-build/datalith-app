---
status: accepted
---

# Separate vault operations, shared GPUI views and native applications into crates

## Context

The desktop binary and the mobile library currently compile the same application modules. This gave the mobile prototype working document editors quickly. It also brought desktop services into the mobile build. For example, mobile skips automatic startup of the local HTTP server, yet the shared settings page can still call the service. Meanwhile, the Markdown reader and editor intentionally share a text buffer so switching modes preserves the current edit.

We want Cargo to enforce which application can use a platform service while allowing both applications to reuse document behavior and GPUI components. This decision records the target structure. The [crate migration plan](../plans/crate-split.md) describes the extraction order and checks.

## Decision

Keep one repository and organize the application around six crates.

| Crate | Responsibility |
|---|---|
| `datalith-core` | Interpret document content independently of GPUI and a vault location. Own formats, parsing, expressions and the rules for interpreting and rewriting wiki links. |
| `datalith-vault` | Apply those rules to a concrete vault. Own file operations, link resolution across documents, catalog maintenance, indexing and search. |
| `datalith-ui` | Own reusable GPUI editors, Markdown and Base views, and shared interactions such as wiki link suggestions. Receive document content and search results from the application. |
| `datalith-desktop` | Compose the desktop application. Own window chrome, menus, desktop navigation, updates and the lifecycle and settings of the local integration server. |
| `datalith-mobile` | Compose the iOS and Android application. Own touch navigation, gestures, software keyboard handling, lifecycle and access to user-selected folders. Adapt layout to the available window space on phones, foldables and tablets. |
| `datalith-server` | Implement the existing loopback HTTP API for the Clipper and other local programs. Its only application consumer is `datalith-desktop`. [Local HTTP server decision](0003-use-a-local-http-server-for-program-integration.md) |

Both native applications consume the shared UI and vault crates. The vault and shared UI can use core types. Core and vault stay independent of GPUI. The two applications build independently of each other. Platform launchers and packaging can retain their existing entry points throughout the migration.

A rename illustrates the boundary. Core knows how to rewrite a wiki link. Vault locates affected documents and applies the rename and link updates. Each application offers the command in its own navigation and reports the outcome. Within the UI, the same editor buffer can serve the desktop, a narrow phone view or a wide tablet view.

## Considered options

- **Keep one application package with internal modules.** This makes early layout experiments inexpensive. Platform exclusions then depend on every command, setting and startup path applying the same rule.
- **Use core and two independent UI crates.** This gives mobile freedom over navigation. Shared editors and viewers would still need an owner, and combining file access with document formats would tie both to the same dependencies.

## Consequences

The extraction adds crate APIs, build wiring and test targets to maintain. Migrate one responsibility at a time with the desktop, iOS and Android builds as checks. Verify that the core and vault dependency trees exclude GPUI and that the mobile dependency tree excludes the Clipper server.

Desktop and mobile keep local file access and offline search. A future self-hosted service can run the same vault operations on its own files and serve a web client. Sync and web delivery are separate from this refactor. [Local and hosted vaults](../plans/local-and-hosted-vaults.md)
