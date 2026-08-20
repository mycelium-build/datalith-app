---
category: format
---

A `.base` file is a YAML **Base Definition**. Opening it shows a read-only **List View**, **Table View**, or **Cards View**; use the Edit/View toggle to edit its YAML source.

```yaml
filters:
  and:
    - 'file.inFolder("Projects")'
    - 'status != "archived"'

properties:
  file.name:
    displayName: Note
  file.mtime:
    displayName: Updated

views:
  - type: table
    name: Recently updated
    limit: 25
    order:
      - file.name
      - status
      - file.mtime
    sort:
      - property: file.mtime
        direction: DESC
    rowHeight: medium
  - type: list
    name: Reading list
    order:
      - file.name
      - author
    markers: bullets
    indentProperties: true
    separators: ", "
  - type: cards
    name: Gallery
    image: cover
    imageFit: cover
    imageAspectRatio: 1.5
    cardSize: 220
    order:
      - file.name
      - cover
      - author
```

# Views

Every Base must declare one or more named views. The first view is selected by default. Use the view switcher to change views without editing the file.

The supported view types are `list`, `table`, and `cards`. View names must be non-empty and unique.

Global `filters` and view `filters` are combined with `AND`.

# Properties

`order` controls displayed property order. When it is omitted, the view shows `file.name`.

The optional `properties` mapping configures display labels:

```yaml
properties:
  file.name:
    displayName: Note
```

Display names are used for table headers and indented list properties.

Supported file properties are:

- `file.name`: file name without its extension. It is rendered as a link to the file.
- `file.path`: normalized relative path.
- `file.ext`: file extension.
- `file.folder`: normalized relative folder.
- `file.size`: file size, compared numerically and displayed as a readable byte value.
- `file.mtime`: modified time, compared numerically and displayed as a stable RFC3339 date and time.
- `file.links`: all resolved outgoing links from the file. Each link is interactive.

Note properties are read from Markdown frontmatter. Scalar values, lists, and nested objects can be displayed. Missing and `null` values are empty. Explicit wikilink values such as `[[Other Note]]` are interactive.

# Filtering

Filters support `and`, `or`, and `not` objects, comparisons using `==`, `!=`, `>`, `>=`, `<`, and `<=`, and scalar values that are strings, numbers, booleans, or `null`.

Property paths may use shorthand, dot notation, or bracket notation:

```yaml
filters:
  and:
    - 'status == "active"'
    - 'note.project.owner == "Romain"'
    - 'note["project status"] != null'
    - 'tags.contains("reading")'
    - 'file.hasTag("reading")'
    - 'file.hasLink("Index")'
```

`file.hasTag()` checks frontmatter tag values already indexed by the catalog.
`file.hasLink()` checks links resolved by the catalog. Unresolved authored links are not included in the first implementation.

# Sorting And Limits

`sort` controls row order and accepts multiple property/direction entries:

```yaml
sort:
  - property: status
    direction: ASC
  - property: file.mtime
    direction: DESC
```

Rows are filtered, sorted, tie-broken by normalized path, and then limited. `limit` must be between 1 and 50,000. A view without an explicit limit is still bounded by the 50,000-row safety ceiling and reports omitted matching files.

# List Settings

List views support:

- `markers`: `bullets`, `numbers`, or `none`.
- `indentProperties`: display properties below the primary item.
- `separators`: text between inline properties.

# Table Settings

Table views support `rowHeight` values of `short`, `medium`, `tall`, and `extra tall`.

# Cards Settings

Cards views support a responsive virtualized grid with these settings:

- `image`: note or file property used for the card image. Local wikilinks, local paths, and HTTP(S) URLs are supported.
- `imageFit`: `cover` or `contain`. The default is `cover`.
- `imageAspectRatio`: positive numeric image aspect ratio. The default is `1`.
- `cardSize`: target card width in pixels. The default is `200`.

The configured `order` properties are shown below the image. `file.name` is an interactive link to the file. Press and hold a card image to show it fullscreen; releasing the mouse returns to the cards view.

All views are read-only. Link cells navigate to files; edit note properties in the Markdown editor.

# Deferred Syntax

The following Bases features are not implemented yet and cause a validation error rather than being silently ignored:

- Formulas and formula properties.
- Date arithmetic, durations, and the complete function library.
- `groupBy` and summaries.
- Embedded Base blocks.
- Maps, plugin-provided layouts, and other view types.
- View-local search, inline property editing, copy/export actions, and creating
  files from a view.
