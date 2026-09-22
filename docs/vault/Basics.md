---
category: guide
---

# Open or create a Vault

A **Vault** is a directory whose files are managed together. Datalith watches it, builds a catalog, and derives search results, wiki-link connections, and graph views.

- **Navigate → Open Vault** opens a folder from disk. Your recent Vaults appear in the vault selector at the bottom of the sidebar.
- The first time Datalith runs, it opens the **Datalith Docs** Vault with [[Welcome]] in reading mode. Later launches restore your last Vault, open tabs, active note, and expanded folders.

# Create a new note

Press **Cmd/Ctrl+N** to create a new file. It is created in the current Vault and immediately renamed, type a name and press **Enter**. The extension decides the file type:

- `My Note.md`: a Markdown file, opened in the editor.
- `Tasks.todotxt`: a todo.txt file, opened in the task editor.
- `Library.base`: a Base Definition, opened as list, table, cards, or graph views.

You can also right-click in the sidebar and choose **New File** or **New Folder**.

# Write some content

To write in a Markdown file, switch to **edit mode** with **Cmd/Ctrl+E** or the **pen icon** in the tab bar. Write normally: headings, lists, bold, code blocks. The [[formats/Markdown]] page lists everything supported.

Add **properties** at the top to tag your note for data-driven views:

```yaml
---
category: ideas
---
```

See [[formats/Properties]] for the details.

# Link your notes

Wiki links are what make a Vault feel connected. In a Markdown file, write `[[My Other Note]]` to create a link. Datalith resolves it automatically. Use `[[My Other Note|display text]]` to set a custom label.

Links between Markdown files become the edges of a [[formats/bases/Graph|graph view]].

# Navigate

- **Cmd/Ctrl+P** opens the quick switcher to jump between open files by name.
- **Cmd/Ctrl+Shift+F** opens the search palette to find files by name or content.

# Edit or view

The **eye icon** in the top-right of the tab bar switches to **reading mode**, and the **pen icon** switches to **edit mode**. You can also use **Cmd/Ctrl+E**.

Datalith remembers this choice for the whole app, including other tabs, files you open next, and future launches. Closing all tabs leaves the workspace empty the next time you open the app.

Some files only have a viewer or only an editor. See [[FileTypes]].

# What else

- The **OS menu bar** has File and Navigate menus with all available actions.
- **Cmd/Ctrl+Shift+D** toggles between light and dark mode. See [[Settings]].
- **Right-click** any file or folder in the sidebar for: New File, New Folder, Rename, Delete, Duplicate, Open in Explorer, and Copy Path.
- See [[Shortcuts]] for the full keybinding reference.
