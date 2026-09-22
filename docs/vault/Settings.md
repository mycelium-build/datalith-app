---
category: reference
---

Open settings from the **Datalith** menu, or press **Cmd/Ctrl+,**.

# Theme

Pick a light theme and a dark theme independently. Both lists are pulled from Datalith's bundled themes, sorted by name.

**Cmd/Ctrl+Shift+D** toggles between light and dark mode using the themes you selected here.

# Font size

A slider from `0.5×` to `3.0×` scales the interface font.

# Fonts

In **Appearance → Fonts**, choose a font for each use:

- **Interface**: menus, tabs, sidebars and controls.
- **Reading**: body text in Markdown previews.
- **Headings**: Markdown titles and headings.
- **Code and editing**: text editors, inline code and code blocks.

Search the fonts installed on your device, including the bundled Pixeloid Sans. Each field shows a preview, and changes apply immediately and persist across restarts. Changing the color theme keeps your chosen fonts.

Select **Default (font name)** in any field to reset that choice. The name shows the font that will be used and updates with the active theme or inherited font. Interface and code use the theme's defaults, reading follows the interface font, and headings use Pixeloid Sans. If a saved font is unavailable, Datalith uses the default and keeps your choice for when the font is available again.

Theme definitions already support `font.family` for the interface and `mono_font.family` for code. The bundled themes currently leave these unset, so the platform defaults apply. Reading and headings do not yet have separate theme fields, and there is no theme editor. Explicit font choices in settings take priority over theme defaults.
