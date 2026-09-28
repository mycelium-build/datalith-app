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

Color resolution is **variant JSON → GPUI component default**. Missing or null colors are passed through to GPUI, which computes a fallback from its mode-specific palette or related colors. Other themes no longer inherit colors or syntax styles from Datalith Light/Dark. Non-color defaults, including fonts, keep their existing behavior. Some optional editor colors remain unset.

The editor labels explicit colors **Theme defined** and component fallbacks **Component Default**. Reset removes only the variant override, restoring the component fallback. Copying a preset preserves its explicit values.

The **Colors** tab contains six base roles: `background`, `foreground`, `muted.background`, `border`, `primary.background`, and `secondary.background`, with no search. It also exposes 19 common overrides: `muted.foreground`, `primary.foreground`, `primary.hover.background`, `primary.active.background`, `secondary.foreground`, `secondary.hover.background`, `secondary.active.background`, `accent.background`, `accent.foreground`, `link`, `selection.background`, `list.active.background`, `list.active.border`, `list.hover.background`, `list.background`, `list.head.background`, `input.border`, `ring`, and `scrollbar.thumb.background`. Rows show the exact JSON key in Colors and Advanced. These settings remain visible even when unset; Reset restores their normal GPUI fallback. The focus-ring key is `ring` and falls back to `blue`. Other component-specific overrides remain in **Advanced**. Advanced includes all 203 recognized color entries, even those also shown in Colors, in a virtualized list with category headings, component-family filtering, search, and independent origin checkboxes.

GPUI Kit's fallbacks apply only when a color is absent: Link and caret use Primary; text/list selection use Primary with their component's opacity treatment; Accent uses Secondary; muted text blends Muted with Foreground; list hover uses Accent; table colors reuse list roles. Explicit component colors always take precedence.

Datalith Dark and macOS Classic Dark explicitly define `link = #419CFF`; other bundled variants do not. All 30 bundled variants explicitly define `list.active.background`. Reset these fields in a custom copy to restore their relationship to Primary. Reset does not rewrite any other explicit colors.

## Effects in current Datalith views

| What to change | Relevant keys |
| --- | --- |
| Main workspace and note text | `background`, `foreground` |
| General action colors and text | `primary.*`, `secondary.*`, `accent.*`; Primary can back links/text selection, Secondary backs Accent, and Accent can back list hover/scrollbar/sidebar accent when their specific colors are unset |
| Muted text, code-block and note-property surfaces, separators | `muted.foreground`, `muted.background`, `border` |
| Links in rendered notes, note properties and Base | `link`, `link.hover`, `link.active` |
| File sidebar | `sidebar.background`, `sidebar.foreground`, `sidebar.border`; file row hover/selection use `list.hover.background` and `list.active.background` through the standard GPUI tree |
| Workspace tabs | `tab.foreground`, `tab.active.background`, `tab.active.foreground`, `tab_bar.background` |
| Window title bar | `title_bar.background` and the library title-bar border |
| Buttons | Default and Primary `button.*` colors for those variants; ghost buttons use `secondary.background`, `secondary.foreground`, and `secondary.active.background` |
| Text fields and selection | `input.border`, `caret`, `ring`, `selection.background` |
| Menus, dialogs, popovers | `popover.*`, `overlay`, `accent.*`, and button colors |
| Tables in notes | `table.background`, `table.head.background`, `table.head.foreground`, `table.row.border`; outer border uses `border` |
| Shortcuts table | General background/text, muted header, and `table.row.border` |
| Base table headers and footers | `table.head.background`, `table.head.foreground`, `table.foot.background`, `table.foot.foreground`; table body uses its existing roles |
| Todo.txt | Shared workspace colors plus success, warning and danger roles; selected tasks use `list.active.background`, while project and context pills are neutral |
| Markdown/YAML source syntax | `highlight.syntax.*`, where the language grammar emits a matching token |
| Source editor surface and current line | `highlight.editor.background`, `highlight.editor.active_line.background` |

The Note/Base/Graph/Todo preview displays the rendered content; Todo.txt is read-only. Syntax colors affect source editing, not code fences in the rendered note.

## Recognized fields with limited or no current effect

- `highlight.editor.foreground` is stored but the current GPUI editor uses `colors.foreground` instead.
- Editor line-number fields are stored; Datalith hides line numbers. Whitespace and gutter settings only matter when those features are visible.
- Diagnostic colors are stored, but Datalith currently supplies no diagnostic provider. GPUI's source editor consumes the error/warning/info/hint foregrounds when diagnostics are present; this does not mean all imported diagnostic backgrounds are painted.
- `group_box.title.foreground` is recognized by the schema but is not applied by GPUI Kit 0.6.1.
- Chart, accordion, group-box, description-list, skeleton, status-bar and tiles colors target components not currently used by Datalith screens.
- `window.border` is Linux-only. `sidebar.primary.*` and `sidebar.accent.*` are not used by Datalith's file rows; their hover and selection use `list.*`.
- `tab.background` is stored, but the current tab component uses the tab-bar surface for inactive tabs. The active tab has its own colors.
- `table.active.*`, `table.even.background`, and `table.hover.background` are not painted by Datalith's current simple/custom tables.
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
