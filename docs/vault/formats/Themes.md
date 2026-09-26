# Theme JSON support

Audited against Datalith's source and GPUI Kit 0.6.1. A key appearing in an upstream theme JSON does not guarantee that Datalith consumes it. Unknown keys are ignored during deserialization and are not preserved when the theme is exported.

## File structure

A file contains `name`, optional `author` and `url`, and a `themes` array. Each entry is a variant with its full `name`, `mode` (`light` or `dark`), `colors`, optional `highlight`, and optional `fonts`.

| Variant fields | Application behavior |
| --- | --- |
| `name`, `mode` | Variant identity and the Light/Dark fallback palette. |
| `colors` | 140 recognized component and palette entries. Their effect depends on which components the application renders. |
| `highlight` | 7 editor entries, 15 diagnostic entries, and 41 optional syntax styles. The editor lists all of them, even when unset. |
| `fonts.interface`, `fonts.reading`, `fonts.headings`, `fonts.code` | Four independent font roles; unavailable fonts remain saved and fall back at render time. |
| `font.family`, `mono_font.family` | Also accepted for interface and code. The role-specific `fonts` value takes precedence; the editor keeps these fields synchronized. |
| `font.size` | Parsed, but the app's interface size is controlled by the Display scale setting. |
| `mono_font.size` | Passed to GPUI Kit for editor typography; rendered Markdown has its own layout sizes. |
| `radius`, `radius.lg`, `shadow` | Applied to library components that use these theme settings. Not exposed as controls in the theme editor. |
| `is_default` | Parsed; it does not choose Datalith's active theme. Active Light/Dark slots are stored in app preferences. |

Syntax styles also accept `font_style` and `font_weight`. Editing a syntax color preserves these attributes. `comment_doc` is the JSON key for documentation comments in the current library; `comment.doc` is the capture name, not a supported JSON key.

## Where a color comes from

Resolution is **variant JSON → bundled Datalith Light/Dark → GPUI component default**. Each missing or null value inherits the corresponding mode-specific Datalith entry. If that entry is also absent, GPUI computes a fallback from its palette or related colors. Some optional editor colors remain unset.

The editor labels these origins **In variant JSON**, **Datalith default**, and **Component default**. Reset removes only the variant override. It does not remove Datalith's inherited value. A theme copied from a preset can already contain many explicit colors: changing `primary.background` or `base.blue` does not override all those explicit values.

## Effects in current Datalith views

| What to change | Relevant keys |
| --- | --- |
| Main workspace and note text | `background`, `foreground` |
| Muted text, code-block surfaces, separators | `muted.foreground`, `muted.background`, `border` |
| Links in rendered notes | `primary.background`; `link` controls library link components instead |
| File sidebar | `sidebar.background`, `sidebar.foreground`, `sidebar.border`; file row hover/selection use `list.*` |
| Workspace tabs | `tab.*`, `tab_bar.background` |
| Window title bar | `title_bar.background` and the library title-bar border |
| Buttons | `button.*`; absent values fall back to matching `primary.*`, `secondary.*`, etc. |
| Text fields and selection | `input.border`, `caret`, `ring`, `selection.background` |
| Menus, dialogs, popovers | `popover.*`, `overlay`, list and button colors |
| Tables in notes and Shortcuts | Library `table.*` fields used by those table components |
| Base tables | Shared workspace colors; their headers and footers use `tab_bar.background`, not `table.head.background` |
| Todo.txt | Shared workspace colors plus success, warning, info and danger roles |
| Markdown/YAML source syntax | `highlight.syntax.*`, where the language grammar emits a matching token |
| Source editor surface and current line | `highlight.editor.background`, `highlight.editor.active_line.background` |

The Note/Base/Todo preview displays the rendered content. Syntax colors affect source editing, not code fences in the rendered note.

## Recognized fields with limited or no current effect

- `highlight.editor.foreground` is stored but the current GPUI editor uses `colors.foreground` instead.
- Editor line-number fields are stored; Datalith hides line numbers. Whitespace and gutter settings only matter when those features are visible.
- Diagnostic colors are stored, but Datalith currently supplies no diagnostic provider. GPUI's source editor consumes the error/warning/info/hint foregrounds when diagnostics are present; this does not mean all imported diagnostic backgrounds are painted.
- `group_box.title.foreground` is recognized by the schema but is not applied by GPUI Kit 0.6.1.
- Chart, accordion, group-box, description-list, skeleton, status-bar and tiles colors target components not currently used by Datalith screens.
- `window.border` is Linux-only. Sidebar primary/accent fields target the library Sidebar component; Datalith's custom file sidebar uses the surface/text/border and `list.*` fields described above.
- Not every syntax token is emitted by every language. A stored token can therefore have no visible occurrence in the current document.

These entries remain accessible in Advanced; they are not presented as essential customization controls.

## Unsupported keys found in bundled upstream JSON

The bundled files still contain upstream data that the current schema ignores. Examples include:

- `colors.panel.background`, `colors.action_bar.background`, `colors.chart.grid`;
- `colors.link.foreground`, `colors.link.hover.foreground`, `colors.link.active.foreground` (the supported keys are `link`, `link.hover`, `link.active`);
- `colors.window_border` (supported: `window.border`);
- `colors.created`, `colors.renamed`, `colors.info`, `colors.editor.active_line_number`;
- Zed diff/status keys under `highlight`: `conflict`, `created`, `deleted`, `hidden`, `ignored`, `modified`, `predictive`, `renamed`, `unreachable`, and their background/border variants.

Advanced is complete for the current typed schema, not for arbitrary keys accepted by other editors. The schema's 140 component colors and 63 highlight color entries come directly from the dependency's serialized types rather than from the subset already defined by a preset.
