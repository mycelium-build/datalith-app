# Datalith

Datalith presents a vault of notes through a customizable desktop workspace.

## Appearance

**Theme**: A named light or dark appearance, including colors and optional font families for the interface, reading, headings, and code.
_Avoid_: Color scheme

**Personal font**: A font chosen for one role in Settings that takes precedence over the active theme.
_Avoid_: Default font

**Theme font**: The font supplied by the active theme for a role. When absent or unavailable, that role uses Datalith's existing fallback.

**Custom theme**: A user-owned theme created from an existing theme and saved independently of its source.

**Theme draft**: Unsaved changes previewed immediately while editing a theme. Leaving the draft requires saving it, discarding it, or continuing to edit.

Theme and Shortcuts are singleton workspace tabs alongside document tabs. Switching tabs retains the theme draft and preview; closing its tab requires resolving unsaved changes. File navigation never replaces a utility tab.
