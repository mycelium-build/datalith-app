---
category: reference
---

Open settings from the **Datalith** menu, or press **Cmd/Ctrl+,**.

# Appearance

Choose **System**, **Light**, or **Dark**. Light and dark theme variants are selected independently. In **System** mode, Datalith follows the operating system and switches between the selected variants.

**Cmd/Ctrl+Shift+D** switches between light and dark mode without changing the selected variants.

# Themes

A theme contains one or more light or dark variants. Bundled themes are read-only, editing one creates a custom copy. Custom themes can be imported, exported, renamed, or deleted. If an imported theme has the same name as an existing theme, Datalith adds **- Copy** to its name.

Deleting a custom theme or variant can be undone.

## Editing a theme

Changes are saved automatically. Changes to the active variant apply immediately across the application. Invalid color values are rejected.

Adding a variant copies the selected variant, including its mode, colors, syntax highlighting, and fonts. Resetting a color removes its explicit value and restores the default derived from the variant's mode and base colors.

Custom themes are stored as individual JSON files in the `themes` folder next to the channel's user `config.json`. Invalid values in a manually edited file are ignored and reported when Datalith starts. The file is left unchanged until the theme is edited in Datalith.

For the JSON format, supported fields, color fallback rules, and current limitations, see [[formats/Themes|Theme format]].

## Fonts

Font choices are stored per variant for four roles: interface, reading, headings, and code and editing. There are no separate personal font overrides.

When a role is not set, Datalith uses these defaults:

| Role | Default |
| --- | --- |
| Interface | Platform interface font |
| Reading | The variant's interface font |
| Headings | Pixeloid Sans |
| Code and editing | Platform code font |

If a selected font is unavailable, its name remains saved and Datalith uses the role's default until the font becomes available.

# Display

Display zoom scales the interface from `0.5×` to `3.0×`. It is independent of the selected theme and fonts.

# Shortcuts

Keyboard shortcut settings are documented in [[Shortcuts]].
