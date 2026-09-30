---
category: reference
---

Open **Settings** from the gear button or the Datalith menu, or press **Cmd/Ctrl+,**. **Appearance** contains the display zoom slider. Use **Manage themes** there to open the separate **Theme** panel without the Settings navigation sidebar.

# Theme

Choose **Light**, **Dark**, or **System** for the application appearance. The light and dark current variants are saved independently. Choosing **Set as Light theme** or **Set as Dark theme** updates only that slot; it does not change the appearance mode. With **System**, the operating system determines which slot is shown.

Search or filter the theme list by **All**, **Light**, **Dark**, or **Custom**. Expand a theme to see its variants and fonts. Custom themes can be edited, renamed, exported, or deleted. Use **Copy & edit** to make a custom copy of a built-in theme. **Import** adds a theme file to Custom; if its name is taken, choose whether to replace an existing custom theme or import a copy. A deleted theme or variant can be restored with **Undo** in the temporary notification.

**Cmd/Ctrl+Shift+D** toggles between light and dark appearance while retaining the current variants.

# Theme editor and fonts

Each custom theme opens in its own workspace tab. Reopening that theme focuses its existing tab. Built-in themes must first be copied with **Copy & edit**. Select a variant and use its pencil button to rename it in a dialog. The × beside its name deletes it; **Add variant** sits below the list. The header pencil renames the whole theme. Each variant has light/dark defaults, colors, and four font roles: **Interface**, **Reading**, **Headings**, and **Code and editing**. The resizable preview uses Datalith's actual Note, Base, Graph, and Todo.txt renderers with the selected variant's resolved colors and fonts. Todo.txt samples are read-only; search, sorting, and navigation remain available. Editing a non-current variant affects only that preview; editing a current variant updates its application slot live.

Colors apply as soon as a value becomes valid. An invalid value is marked beside its field and is restored to the last valid value when the field loses focus. **Reset** removes a color override and restores GPUI Kit's fallback for the variant's mode and current base colors. Changes autosave; the editor reports **Saving**, **Autosaved**, or **Couldn’t save** with **Retry** when necessary. Closing its tab does not discard valid changes.

**Add variant** clones the selected variant, including its colors, syntax highlighting, mode, and fonts. When adding a second variant, name both variants in the dialog. Choose **Apply fonts to all variants** to copy the selected variant's font roles across the theme.

The **Fonts** tab shows a muted sample above each selector. **Default (font name)** identifies the effective fallback, including the reading font following the interface font.

Font choices belong to each theme variant. There are no personal font overrides in Appearance. An unset role uses Datalith's defaults: interface and code follow the platform, reading follows interface, and headings use Pixeloid Sans. An unavailable font remains saved and is visibly marked; rendering uses that role's fallback until the font becomes available.

Custom themes are individual files in the `themes` folder beside the channel's user `config.json`. Each file holds one theme with all its variants.

# Understanding theme colors

**Colors** keeps six base colors without search: **Background**, **Text**, **Muted surface**, **Borders**, **Primary**, and **Secondary**. Below them, common overrides are grouped by their role:

- **Primary & selection**: text on Primary, its hover and pressed states, links, text selection, selected rows and their border.
- **Secondary & accent**: text on Secondary, its custom title-bar hover and selected/open states, Accent and its text, hovered rows, and scrollbar thumbs.
- **Text & surfaces**: muted text, input borders and focus rings, list background, and the list-header color inherited by table headers and summary footers.

**Advanced** lists all **203** colors recognized by the current schema, including all Colors controls and unset entries, under category headings. Only visible rows are rendered; scrolling reaches the entire list from the first opening and after resizing. Use **Component family** to isolate Sidebar, Buttons, Tables, or another family. Search narrows the result further. The two origin checkboxes are independent and both are checked initially. Its counter counts matching colors, excluding category headings. **Clear filters** restores all families, all origins, and an empty search.

- **Theme defined**: explicitly saved in this variant.
- **Component Default**: calculated by GPUI Kit, or unset when the component has no explicit fallback color.

A specific component color takes precedence over its base color. **Reset** removes that override and restores the component fallback. Source-editor and syntax colors affect source editing, not rendered Markdown. Some library fields are stored but have no effect in current Datalith screens; their descriptions explain this.

Rendered links use **Link**, which follows **Primary** when unset in both light and dark variants. Datalith Dark and macOS Classic Dark explicitly define Link, so copies retain that override until Reset. All bundled variants explicitly define the list selection color; reset `list.active.background` to make it follow Primary. The list hover color follows Accent instead.

For the supported JSON fields and current limitations, see [Theme format](formats/Themes.md).

# Display

The zoom slider scales the interface from `0.5×` to `3.0×` independently of the selected theme and its fonts.

# Shortcuts

Open **Shortcuts** from the gear or Datalith menu, or press **Cmd/Ctrl+/**. It opens a read-only reference tab with actions grouped by category and their current platform shortcuts.
