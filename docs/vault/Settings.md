---
category: reference
---

The gear button opens **Settings…**, **Theme…**, and **Shortcuts…**. Open Settings from this menu or the **Datalith** menu, or press **Cmd/Ctrl+,**. Theme and Shortcuts open in dedicated workspace tabs. Shortcuts remain available with **Cmd/Ctrl+/**. Opening either editor again selects its existing tab.

# Theme

Pick a light theme and a dark theme independently. Both lists include bundled and saved custom themes, sorted by name.

**Cmd/Ctrl+Shift+D** toggles between light and dark mode using the themes you selected here.

# Font size

A slider from `0.5×` to `3.0×` scales the interface font.

# Fonts

In **Appearance → Fonts**, choose a font for each use:

- **Interface**: menus, tabs, sidebars and controls.
- **Reading**: body text in Markdown previews.
- **Headings**: Markdown titles and headings.
- **Code and editing**: text editors, inline code and code blocks.

Search the fonts installed on your device, including the bundled Pixeloid Sans. Each field shows a preview, and changes apply immediately and persist across restarts.

Select **Theme (font name)** to follow the active theme for that role. A personal font takes priority. If a theme does not define a usable font, interface and code use the platform defaults, reading follows the interface font, and headings use Pixeloid Sans. Unavailable font choices are kept so they work again when the font becomes available.

When selecting a theme that defines fonts while personal fonts are active, choose **Use theme fonts** to return all four roles to Theme, or **Keep personal fonts** to preserve your choices. Automatic system appearance changes preserve personal fonts.

# Theme editor

Open **Theme…** from the gear or Datalith menu. The editor opens in the document tab bar with the active theme and previews color and font changes immediately throughout the workspace. Personal font overrides still take priority; **Use theme fonts** explicitly resets them.

- Select any light or dark theme to apply it and use it as a starting point.
- Choose a color swatch or search the full list of theme colors. Enter a color or use the color picker. **Reset** removes that color override and uses the light/dark mode default.
- Define each of the four font roles. **Application default** leaves the role undefined.
- **Save theme** saves the draft under **Save as**. Bundled themes require a new name; saving an existing custom theme under its current name updates it. A different name creates another copy.
- **Create from current…** starts a named copy of the current saved theme.

Switching between tabs keeps the draft and its live preview, so you can check the result in a note. An unsaved draft is marked with a dot in the Theme tab. Close editor tabs with their close button or **Cmd/Ctrl+W**; Escape only dismisses a menu or cancels a pending close decision.

Closing the Theme tab, creating another theme, or changing the theme or mode with unsaved changes offers **Save**, **Discard**, and **Continue editing**. Discard restores the saved appearance. Drafts are not retained after closing the application.

Custom themes are independent files in the `themes` subfolder beside your user `config.json`, separate for each Datalith release channel. They load automatically on startup. Each file contains a versioned theme document with colors, the original syntax highlighting, and optional `fonts.interface`, `fonts.reading`, `fonts.headings`, and `fonts.code` families. The legacy `font.family` and `mono_font.family` fields remain supported. Saving a theme preserves your interface zoom.
