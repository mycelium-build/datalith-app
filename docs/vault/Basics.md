---
category: guide
---

# Basics

## Vaults

A **Vault** is a directory whose files are managed together. Datalith watches the folder, builds a catalog of your files, and derives things from it: search results, wiki-link connections, and graph views.

- Open a Vault with **Open Vault** from the navigate menu.
- Your recent Vaults are listed in the vault selector at the bottom of the sidebar.
- The first time Datalith runs, it opens the **Datalith Docs** Vault you are reading now.

## Files, folders, and the sidebar

The sidebar shows the file tree of the current Vault. .

- **Enter** on a folder expands or collapses it.
- **Enter** on a file opens it; hold **⌘** to open in a new tab.
- Use **⌘0** to focus the sidebar and the arrow keys to move through the tree.

## Tabs

Each open file lives in a tab. `⌘1`-`⌘9` jump to a tab, `⌘T` opens a new tab, and `⌘W` closes the current one. Use **⌘[** and **⌘]** to walk back and forward through your navigation history.

## Tracked files

Datalith works with registered file types: Markdown, todo.txt, Graph Definitions, and images. See [[FileTypes]] for the full capabilities summary, and the [[formats/TodoTxt|todo.txt]] and [[formats/Graph|graph]] pages for details. Each type declares which capabilities it supports, for example whether it can contain [[formats/Properties|properties]] or participate in [[Search|text search]].

## Everything is plain text

Your Vault is authoritative. Datalith's catalog and indexes are disposable and rebuilt from the files, so editing files outside Datalith is fine.
