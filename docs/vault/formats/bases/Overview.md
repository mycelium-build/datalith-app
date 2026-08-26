---
category: format
---

A `.base` file is a YAML **Base Definition**. Opening it shows one of its views — a read-only **List View**, **Table View**, **Cards View**, or **Graph View**; use the Edit/View toggle to edit its YAML source.

```yaml
formulas:
  ppu: (price / count).toFixed(2)

summaries:
  roundedAverage: values.mean().round(2)

filters:
  and:
    - 'file.inFolder("Projects")'
    - 'status != "archived"'
    - 'file.mtime > now() - "1 week"'

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
      - formula.ppu
      - file.mtime
    sort:
      - property: file.mtime
        direction: DESC
    groupBy:
      property: status
      direction: ASC
  - type: graph
    name: Link map
```

This page covers everything **shared** by all view types. Each view type documents its particular settings on its own page:

- [[formats/bases/List|List]]
- [[formats/bases/Table|Table]]
- [[formats/bases/Cards|Cards]]
- [[formats/bases/Graph|Graph]]

# Views

Every Base must declare one or more named views under `views`. The first view is selected by default; use the view switcher to change views without editing the file.

The supported view types are `list`, `table`, `cards`, and `graph`. View names must be non-empty and unique.

Global `filters` and per-view `filters` are combined with `AND`.

Settings that belong to another view type are silently ignored, so a stray `rowHeight:` on a list view is harmless. Unknown keys still fail validation, so typos are never swallowed.

# Properties

`order` controls displayed property order. When it is omitted, the view shows `file.name`.

The optional `properties` mapping configures display labels:

```yaml
properties:
  file.name:
    displayName: Note
```

A **display name** is used at every label site: table headers, indented list property labels, summary labels, and card property labels. Without one, the label falls back to the property path's last segment.

## Formula properties

The `formulas` mapping declares named formulas usable anywhere a property can appear — in `order`, `sort`, `groupBy`, `summaries`, filters, and even inside other formulas (circular references are rejected):

```yaml
formulas:
  ppu: (price / count).toFixed(2)
```

Formula names are referenced with `formula.<name>` or the shorthand shown above.

Supported file properties are:

- `file.name`: file name without its extension. It is rendered as a link to the file.
- `file.path`: normalized relative path.
- `file.ext`: file extension.
- `file.folder`: normalized relative folder.
- `file.size`: file size, displayed as a readable byte value.
- `file.mtime`: modified time, displayed as an ISO-8601 UTC date and time.
- `file.ctime`: created time, same display as `file.mtime`.
- `file.links`: all resolved outgoing links from the file. Each link is interactive.
- `file.embeds`: resolved embed targets (`![[...]]`) from the file.
- `file.backlinks`: files whose resolved links point here.
- `file.tags`: tags from YAML frontmatter.
- `file.properties`: the complete frontmatter object.

Note properties are read from Markdown frontmatter. Scalar values, lists, and nested objects can be displayed. Missing and `null` values are empty. Explicit wikilink values such as `[[Other Note]]` are interactive.

# Expressions

Filters, formulas, custom summaries, class filters, and date arithmetic share one expression language. Expressions support arithmetic (`+`, `-`, `*`, `/`, `%`, parentheses), inline boolean operators (`!`, `&&`, `||`), comparisons (`==`, `!=`, `>`, `>=`, `<`, `<=`), string/number/boolean/null literals, and function calls.

Property paths may use shorthand, dot notation, or bracket notation:

```yaml
filters:
  and:
    - 'status == "active"'
    - 'note.project.owner == "Romain"'
    - 'note["project status"] != null'
    - 'price > 5 && !archived'
    - 'tags.contains("reading")'
    - 'name.startsWith("Read")'
    - 'file.hasTag("reading")'
    - 'file.hasLink("Index")'
    - '(price / age) > 5'
    - 'if(done, file.size, 0) > 1024'
```

Structured filter objects are also accepted: `{and: [...]}`, `{or: [...]}`, and `{not: ...}` wrap expression strings.

Supported functions include `if`, `min`, `max`, `abs`, `round`, `length`, `lower`, `upper`, `trim`, `ltrim`, `rtrim`, `replace`, `substr`, `instr`, and the date functions `date()`, `datetime()`, `time()`, and `julianday()`. Supported methods include `.contains()`, `.startsWith()`, `.endsWith()`, `.toFixed(n)`, `.round(n)`, `.lower()`, `.upper()`, `.trim()`, `.date()`, and moment-style `.format("YYYY-MM-DD")`.

Functions outside this whitelist fail Base validation with a precise error rather than being approximated.

# Dates And Durations

Dates are canonical ISO-8601 UTC values; `file.mtime` and `file.ctime` compare chronologically against them. Duration literals combine with dates using `+` and `-`:

```yaml
filters:
  - 'file.mtime > now() - "1 week"'
  - 'file.ctime + "1M" < today()'
```

Duration units are `y`, `M` (months), `w`, `d`, `h`, `m` (minutes), and `s`. Subtracting two dates yields milliseconds. `now()` and `today()` are frozen once per refresh so results stay internally consistent. All date handling is UTC.

# Grouping

A list, table, or cards view may group rows by one property:

```yaml
groupBy:
  property: status
  direction: ASC
```

Groups are ordered by their key value using `groupBy.direction`; rows within each group follow `sort`. Each view type renders a non-interactive header showing the key value and row count. Missing or empty keys form an "Empty" group rendered last. `limit` counts rows only — headers never consume it.

Graph views do not use `groupBy`; they classify nodes with [[formats/bases/Graph|classes]] instead.

# Summaries

Default summaries are selected by name in a view's `summaries` mapping: `Average`, `Min`, `Max`, `Sum`, `Range`, `Median`, `Stddev`, `Earliest`, `Latest`, `Checked`, `Unchecked`, `Empty`, `Filled`, and `Unique`. `Range` uses days for date-typed sources like `file.mtime`.

Custom summaries are declared globally from the `values` keyword and must reduce to one aggregate plus optional rounding:

```yaml
summaries:
  roundedAverage: values.mean().round(2)
views:
  - type: table
    name: T
    summaries:
      price: Average
      score: roundedAverage
```

Summaries aggregate every matching row, before `limit`. Tables render a footer under the summarized columns; lists and cards render a compact summary strip beneath the content. Summaries always aggregate the whole result set, not per-group.

# Sorting And Limits

`sort` controls row order and accepts multiple property/direction entries:

```yaml
sort:
  - property: status
    direction: ASC
  - property: file.mtime
    direction: DESC
```

Rows are filtered, sorted, tie-broken by normalized path, and then limited. `limit` must be between 1 and 50,000. A view without an explicit limit is still bounded by the 50,000-row safety ceiling, and the view reports how many matching files were omitted.

# Deferred Syntax

The following Bases features cause a validation error rather than being silently ignored:

- Embedded Base blocks in Markdown.
- Map layouts and plugin-provided view types.
- Dynamic durations stored in properties (duration literals work).
- Custom summaries beyond aggregate-plus-rounding chains.
- Per-group summary footers.
- Fan-out of list-valued group keys into multiple groups.
- The `this` context for bases opened from another file or sidebar.
- View-local search, inline property editing, copy/export actions, and creating files from a view.
