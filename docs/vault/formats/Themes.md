A theme is a JSON file containing one or more light or dark variants, each with its own colors, syntax styles, and fonts. Use this reference to create a theme file or adapt one from another editor. For choosing themes, using the theme editor, and importing or exporting files, see [Settings](Settings.md#theme).

# File structure

Create a `.json` file with a top-level object containing the theme metadata and a `themes` array:

```json
{
  "name": "My Theme",
  "author": "Your name",
  "themes": [
    {
      "name": "My Theme Light",
      "mode": "light",
      "colors": {
        "background": "#FAFAFA",
        "foreground": "#202020",
        "primary.background": "#2864DC",
        "link": null
      },
      "highlight": {
        "editor.background": "#FAFAFA",
        "editor.active_line.background": "#2864DC15",
        "syntax": {
          "keyword": { "color": "#2864DC" },
          "comment_doc": {
            "color": "#607060",
            "font_style": "italic",
            "font_weight": 400
          }
        }
      },
      "fonts": {
        "interface": "Inter",
        "reading": "Georgia",
        "headings": "Inter",
        "code": "JetBrains Mono"
      }
    },
    {
      "name": "My Theme Dark",
      "mode": "dark",
      "colors": {
        "background": "#202020",
        "foreground": "#FAFAFA",
        "primary.background": "#80AAFF"
      }
    }
  ]
}
```

The example defines two independent variants. The dark variant uses defaults for its omitted colors, syntax styles, and fonts; it does not inherit the light variant's settings.

| Top-level field | Value |
| --- | --- |
| `name` | Theme name. |
| `author` | Optional author name. |
| `url` | Optional project or author URL. |
| `themes` | Array of variant objects. |

For a single variant, use the theme name as its name. For multiple variants, prefix each name with the theme name followed by a space, such as `My Theme Light` and `My Theme Dark`.

## Variant fields

| Field | Value and usage |
| --- | --- |
| `name` | Full variant name. |
| `mode` | `"light"` or `"dark"`; selects the fallback palette. |
| `colors` | Object containing component and palette colors. There are 140 recognized entries. |
| `highlight` | Optional object containing 7 source-editor colors, 15 diagnostic colors, and a `syntax` object with 41 optional styles. |
| `fonts` | Optional object with `interface`, `reading`, `headings`, and `code` font-family strings. |
| `font.family`, `mono_font.family` | Alternative interface and code font-family strings. The corresponding `fonts` value takes precedence; the theme editor keeps these fields synchronized. |
| `font.size` | Accepted, but interface size is controlled by the display zoom setting. |
| `mono_font.size` | Source-editor typography size. Rendered Markdown uses its own layout sizes. |
| `radius`, `radius.lg`, `shadow` | Corner radii and shadow settings for library components that use them. These settings have no controls in the theme editor. |
| `is_default` | Accepted, but does not activate a variant. Choose active light/dark variants in the theme panel. |

Keys containing dots are literal JSON keys. For example, put `"primary.background"` inside `colors`, not a nested `primary` object. Likewise, `"font.family"` belongs directly in the variant object. `fonts` and `highlight.syntax`, however, are nested objects as shown above.

## Define colors

Put component colors inside `colors`. Use hexadecimal strings such as `"#2864DC"`; an eight-digit value such as `"#2864DC15"` includes an alpha channel.

Start with the six base roles: `background`, `foreground`, `muted.background`, `border`, `primary.background`, and `secondary.background`. Add component overrides only where you want a different color.

Omit a color, or set it to `null`, to use its component default. Defaults come from the variant's mode-specific palette or related colors. Some optional editor colors remain unset. Variants do not inherit colors or syntax styles from Datalith Light/Dark or other variants.

Specific component colors take precedence over base colors. When unset:

- `link` and `caret` follow Primary.
- Text and list selection follow Primary with their component's opacity treatment.
- Accent follows Secondary, and list hover follows Accent.
- Muted text blends the muted surface with the foreground.
- Table colors reuse list roles.
- `ring`, the focus-ring color, falls back to `blue`.

If a copied theme's links or selected rows do not follow Primary, remove their explicit overrides or set them to `null`. Datalith Dark and macOS Classic Dark define `link = #419CFF`, and all bundled variants define `list.active.background`. Removing one override does not change other explicit colors.

### Find the right key

| To customize | Keys to adjust |
| --- | --- |
| Workspace background and note text | `background`, `foreground` |
| Action colors | `primary.*`, `secondary.*`, `accent.*` |
| Muted text, code-block and note-property surfaces, separators | `muted.foreground`, `muted.background`, `border` |
| Links in notes, note properties, and Base | `link`, `link.hover`, `link.active` |
| File sidebar | `sidebar.background`, `sidebar.foreground`, `sidebar.border` |
| Hovered and selected file rows | `list.hover.background`, `list.active.background`, `list.active.border` |
| Workspace tabs | `tab.foreground`, `tab.active.background`, `tab.active.foreground`, `tab_bar.background` |
| Window title bar | `title_bar.background` |
| Default and Primary buttons | The corresponding `button.*` colors |
| Ghost buttons | `secondary.background`, `secondary.foreground`, `secondary.active.background` |
| Text fields, caret, focus rings, and text selection | `input.border`, `caret`, `ring`, `selection.background` |
| Menus, dialogs, and popovers | `popover.*`, `overlay`, `accent.*`, and button colors |
| Tables in notes | `table.background`, `table.head.background`, `table.head.foreground`, `table.row.border`; `border` for the outer border |
| Base table headers and footers | `table.head.background`, `table.head.foreground`, `table.foot.background`, `table.foot.foreground` |
| Todo.txt | Workspace colors, success/warning/danger roles, and `list.active.background` for selected tasks |
| Markdown and YAML source syntax | `highlight.syntax.*` |
| Source-editor background and current line | `highlight.editor.background`, `highlight.editor.active_line.background` |

## Define source highlighting

Put source-editor and diagnostic colors directly inside `highlight`, using literal keys such as `"editor.background"` and `"editor.active_line.background"`. Put token styles inside its nested `syntax` object.

Each syntax style is an object with an optional `color`, `font_style`, and `font_weight`. For example:

```json
"syntax": {
  "comment": { "color": "#607060", "font_style": "italic" },
  "keyword": { "color": "#2864DC", "font_weight": 700 }
}
```

Use `comment_doc` for documentation comments. `comment.doc` is a syntax capture name, not a supported JSON key. Changing a syntax color in the theme editor preserves its font style and weight.

Syntax colors apply to source editing, not code fences in rendered notes. A syntax style appears only when the document's language grammar emits the matching token.

## Define fonts

Put font-family names inside `fonts`, using the four roles shown in the file example. Each role is independent and belongs to its variant. Omit a role or set it to `null` to use its default:

| Role | Default |
| --- | --- |
| `interface` | Platform interface font. |
| `reading` | The variant's interface font. |
| `headings` | Pixeloid Sans. |
| `code` | Platform code font. |

If a font is unavailable, its name remains saved and rendering uses that role's fallback until it becomes available. Use the display zoom setting to resize the interface; `font.size` does not control it.

## Compatibility and limitations

The **Advanced** tab lists all 203 recognized color entries: 140 component colors and 63 highlight colors, including unset fields. Use it to find supported keys, then place component keys in `colors` and highlight keys in `highlight` (or `highlight.syntax` for token styles). Some fields can be saved without producing a visible change:

- `highlight.editor.foreground`: the source editor uses `colors.foreground`.
- Line-number, whitespace, and gutter colors: these require the corresponding features to be visible; Datalith hides line numbers.
- Diagnostic colors: Datalith does not supply a diagnostic provider.
- Chart, accordion, group-box, description-list, skeleton, status-bar, and tiles colors: Datalith's screens do not use these components. `group_box.title.foreground` is recognized but not applied by the component library.
- `window.border`: applies only on Linux.
- `sidebar.primary.*` and `sidebar.accent.*`: file-row hover and selection use `list.*` instead.
- `tab.background`: inactive tabs use the tab-bar surface.
- `table.active.*`, `table.even.background`, and `table.hover.background`: Datalith's tables do not use these colors.

When importing a theme from another editor, use Datalith's supported key names:

| Use | Instead of |
| --- | --- |
| `colors.link` | `colors.link.foreground` |
| `colors.link.hover` | `colors.link.hover.foreground` |
| `colors.link.active` | `colors.link.active.foreground` |
| `colors.window.border` | `colors.window_border` |

Unknown keys are ignored on import and omitted on export. This includes `colors.panel.background`, `colors.action_bar.background`, `colors.chart.grid`, `colors.created`, `colors.renamed`, `colors.info`, and `colors.editor.active_line_number`, as well as Zed diff/status fields under `highlight` such as `conflict`, `created`, `deleted`, `hidden`, `ignored`, `modified`, `predictive`, `renamed`, and `unreachable`, including their background and border variants.
