---
category: reference
---

# Settings

Open **Settings** from the gear button or the Datalith menu, or press **Cmd/Ctrl+,**. **Appearance** contains the display zoom slider. Use **Manage themes** there to open the separate **Theme** panel without the Settings navigation sidebar.

## Theme

Choose **Light**, **Dark**, or **System** for the application appearance. The light and dark current variants are saved independently. Choosing **Set as Light theme** or **Set as Dark theme** updates only that slot; it does not change the appearance mode. With **System**, the operating system determines which slot is shown.

Search or filter the theme list by **All**, **Light**, **Dark**, or **Custom**. Expand a theme to see its variants and fonts. Custom themes can be edited, renamed, exported, or deleted. Use **Copy & edit** to make a custom copy of a built-in theme. **Import** adds a theme file to Custom; if its name is taken, choose whether to replace an existing custom theme or import a copy. A deleted theme or variant can be restored with **Undo** in the temporary notification.

**Cmd/Ctrl+Shift+D** toggles between light and dark appearance while retaining the current variants.

## Theme editor and fonts

Each custom theme opens in its own workspace tab. Reopening that theme focuses its existing tab. Built-in themes must first be copied with **Copy & edit**. Select a variant to edit its suffix, light/dark mode, grouped color and syntax tokens, and four font roles: **Interface**, **Reading**, **Headings**, and **Code and editing**. The resizable preview uses Datalith's actual Markdown document renderer with the selected variant's resolved colors and fonts. Editing a non-current variant affects only that preview; editing a current variant updates its application slot live.

Colors apply as soon as a value becomes valid. An invalid value is marked beside its field and is restored to the last valid value when the field loses focus. **Reset** removes a token override and restores the Datalith default for the variant's mode. Changes autosave; the editor reports **Saving**, **Autosaved**, or **Couldn’t save** with **Retry** when necessary. Closing its tab does not discard valid changes.

**Add variant** clones the selected variant, including its colors, syntax highlighting, mode, and fonts. When adding a second variant, name both variants in the dialog. Choose **Apply these fonts to all variants** to copy the selected variant's font roles across the theme.

Font choices belong to each theme variant. There are no personal font overrides in Appearance. An unset role uses Datalith's defaults: interface and code follow the platform, reading follows interface, and headings use Pixeloid Sans. An unavailable font remains saved and is visibly marked; rendering uses that role's fallback until the font becomes available.

Custom themes are individual files in the `themes` folder beside the channel's user `config.json`. Each file holds one theme with all its variants.

## Display

The zoom slider scales the interface from `0.5×` to `3.0×` independently of the selected theme and its fonts.

## Shortcuts

Open **Shortcuts** from the gear or Datalith menu, or press **Cmd/Ctrl+/**. It opens a read-only reference tab with actions grouped by category and their current platform shortcuts.
