---
category: guide
---

# Basics

## Open or create a Vault

A **Vault** is a directory whose files are managed together. Datalith watches it, builds a catalog, and derives search results, wiki-link connections, and graph views.

- **Navigate → Open Vault** opens a folder from disk. Your recent Vaults appear in the vault selector at the bottom of the sidebar.
- The first time Datalith runs, it opens the **Datalith Docs** Vault you are reading now.

## Create a new note

Press **⌘N** to create a new file. It is created in the current Vault and immediately renamed, type a name and press **Enter**. The extension decides the file type:

- `My Note.md`: a Markdown file, opened in the editor.
- `Tasks.todotxt`: a todo.txt file, opened in the task editor.
- `My Graph.graph`: a Graph Definition, opened as a Graph View.

You can also right-click in the sidebar and choose **New File** or **New Folder**.

## Write some content

If you created a Markdown file, you are in **edit mode** by default. Write normally: headings, lists, bold, code blocks. The [[formats/Markdown]] page lists everything supported.

Add **properties** at the top to tag your note for data-driven views:

```yaml
---
category: ideas
---
```

See [[formats/Properties]] for the details.

## Link your notes

Wiki links are what make a Vault feel connected. In a Markdown file, write `[[My Other Note]]` to create a link. Datalith resolves it automatically. Use `[[My Other Note|display text]]` to set a custom label.

Links between Markdown files become the edges of a [[formats/Graph|graph]].

## Navigate

- **⌘P** opens the quick switcher to jump between open files by name.
- **⌘⇧F** opens the search palette to find files by name or content.

## Edit or view

The **eye icon** in the top-right of the tab bar toggles between **edit** mode and **view** mode, use **⌘E** to switch.

Some files only have a viewer or only an editor. See [[FileTypes]].

## What else

- The **OS menu bar** has File and Navigate menus with all available actions.
- **⌘⇧D** toggles between light and dark mode. See [[Settings]].
- **Right-click** any file or folder in the sidebar for: New File, New Folder, Rename, Delete, Duplicate, Open in Explorer, and Copy Path.
- See [[Shortcuts]] for the full keybinding reference.
