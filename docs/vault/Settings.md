---
category: reference
---

Open **Settings** from the gear button or the Datalith menu, or press **Cmd/Ctrl+,**. **Appearance** contains the display zoom slider. Use **Manage themes** there to open the separate **Theme** panel without the Settings navigation sidebar.

# Theme

Choose **System**, **Light**, or **Dark** for the application appearance; the current light and dark themes are shown beside the toggle. The light and dark current variants are saved independently. With **System**, the operating system determines which one is shown, and the list offers both light and dark variants. With **Light** or **Dark**, the list offers only variants of that mode.

Each theme is a card with a miniature window drawn in the variant's own colors and fonts. The buttons below it switch which variant the miniature shows. Click the miniature, or focus it and press **Enter**, to make that variant the current light or dark theme for its mode; the current one has an accent border and a **Light** or **Dark** badge. Search narrows the cards by theme or variant name.

Custom themes and built-in themes are listed separately. The pencil on a built-in theme creates an editable copy named after it with **- Copy** and opens it in the theme editor. A custom theme's **…** menu opens it in the editor (**Customize**), exports it, or deletes it; rename themes in the editor. **Import…** adds a theme file to Custom; if its name is already taken, the imported theme is named with **- Copy**. A deleted theme or variant can be restored with **Undo** in the temporary notification.

**Cmd/Ctrl+Shift+D** toggles between light and dark appearance while retaining the current variants.

# Theme editor and fonts

Each custom theme opens in its own workspace tab. Reopening that theme focuses its existing tab. To edit a built-in theme, first copy it with its pencil in the Theme panel. Variants appear as miniature windows drawn in their own colors and fonts, with a sun or moon for their mode; they form a column beside the controls, or a row above them in a narrow window. The pencil beside a variant's name renames it and the × deletes it; **Add variant** follows the list. The header pencil renames the whole theme. Each variant has light/dark defaults, colors, and four font roles: **Interface**, **Reading**, **Headings**, and **Code and editing**. The resizable preview uses Datalith's actual Note, Base, Graph, and Todo.txt renderers with the selected variant's resolved colors and fonts. Todo.txt samples are read-only; search, sorting, and navigation remain available. Editing a non-current variant affects only that preview; editing a current variant updates its application slot live.

Colors apply as soon as a value becomes valid. An invalid value is marked beside its field and is restored to the last valid value when the field loses focus. **Reset** removes a color override and restores GPUI Kit's fallback for the variant's mode and current base colors. Changes autosave; the editor reports **Saving**, **Autosaved**, or **Couldn’t save** with **Retry** when necessary. Closing its tab does not discard valid changes.

**Add variant** clones the selected variant, including its colors, syntax highlighting, mode, and fonts. When adding a second variant, name both variants in the dialog. Choose **Apply fonts to all variants** to copy the selected variant's font roles across the theme.

The **Fonts** tab shows each role at its real size in the variant's colors above its selector: a heading, a paragraph, a code block, and an interface button with labels. **Default (font name)** identifies the effective fallback, including the reading font following the interface font.

Font choices belong to each theme variant. There are no personal font overrides in Appearance. An unset role uses Datalith's defaults: interface and code follow the platform, reading follows interface, and headings use Pixeloid Sans. An unavailable font remains saved and is visibly marked; rendering uses that role's fallback until the font becomes available.

Custom themes are individual files in the `themes` folder beside the channel's user `config.json`. Each file holds one theme with all its variants.

# Understanding theme colors

**Colors** starts with **Appearance**, a **Light** / **Dark** toggle for whether the variant is used in light or dark mode, then the six base colors as swatches: **Background**, **Text**, **Muted surface**, **Borders**, **Primary**, and **Secondary**; select a swatch to pick its color. Below them, common overrides are grouped by their role. Each group opens with samples drawn in the variant's resolved colors, such as a primary button at rest, hovered and pressed, so the effect of a change is visible next to its control:

- **Primary & selection**: text on Primary, its hover and pressed states, links, text selection, selected rows and their border.
- **Secondary & accent**: text on Secondary, its custom title-bar hover and selected/open states, Accent and its text, hovered rows, and scrollbar thumbs.
- **Text & surfaces**: muted text, input borders and focus rings, list background, and the list-header color inherited by table headers and summary footers.

**All colors** lists all **203** colors recognized by the current schema, including all Colors controls and unset entries, under category headings. Each color takes one line: its name and token, value, origin, and a reset button; hover the name for its description. Only visible rows are rendered; scrolling reaches the entire list from the first opening and after resizing. Use **Component family** to isolate Sidebar, Buttons, Tables, or another family. Search narrows the result further. The two origin checkboxes are independent and both are checked initially. Its counter counts matching colors, excluding category headings. **Clear filters** restores all families, all origins, and an empty search.

- **Set in theme** (**Set** on a row): explicitly saved in this variant.
- **Automatic** (**Auto** on a row): calculated by GPUI Kit, or unset when the component has no explicit fallback color.

A specific component color takes precedence over its base color. **Reset** removes that override and restores the component fallback. Source-editor and syntax colors affect source editing, not rendered Markdown. Some library fields are stored but have no effect in current Datalith screens; their descriptions explain this.

Rendered links use **Link**, which follows **Primary** when unset in both light and dark variants. Datalith Dark and macOS Classic Dark explicitly define Link, so copies retain that override until Reset. All bundled variants explicitly define the list selection color; reset `list.active.background` to make it follow Primary. The list hover color follows Accent instead.

For the supported JSON fields and current limitations, see [Theme format](formats/Themes.md).

# Display

The zoom slider scales the interface from `0.5×` to `3.0×` independently of the selected theme and its fonts.

# Shortcuts

Open **Shortcuts** from the gear or Datalith menu, or press **Cmd/Ctrl+/**. It opens a read-only reference tab with actions grouped by category and their current platform shortcuts.
