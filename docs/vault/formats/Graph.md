---
category: format
---

# Graph Definitions

A `.graph` file is a YAML **Graph Definition**. Opening it shows a derived **Graph View**; use the Edit/View toggle to edit its YAML source.

```yaml
limit: 2000

filters:
  and:
    - 'file.inFolder("Inbox")'
    - 'status != "archived"'
    - 'priority >= 3'
    - 'tags.contains("project")'

groups:
  - name: Done
    filters: 'status == "done"'
    node:
      color: '#ff0000cc'
      size: 1.25

display:
  node:
    color: '#7c8cff'
    size: 1.0
    propertional: true
    border:
      color: '#ffffff80'
      width: 1.0
    hover:
      color: '#9aa5ff'
      size: 1.25
      border:
        color: '#ffffff'
        width: 2.0
  edge:
    color: 'oklch(70% 0.02 260 / 45%)'
    width: 1.0
    arrow: false
    hover:
      direction:
        outgoing:
          color: '#9aa5ff'
          width: 2.0
        incoming:
          color: '#ff9ad5'
          width: 1.5
        both:
          color: '#c69aff'
          width: 2.5
  orphan:
    show: true
    node:
      color: 'hsl(220 10% 60%)'
      size: 0.8
      border:
        color: '#ffffff40'
        width: 1.0
      hover:
        color: 'hsl(220 10% 70%)'
        size: 1.25
        border:
          color: '#ffffff'
          width: 2.0
physics:
  center:
    strength: 0.002
  repulsion:
    strength: 1024.0
  link:
    strength: 0.04
    distance: 128.0
```

## Filters

Omitting `filters`, using `filters: []`, or using an empty `and` selects every Markdown file. An empty `or` selects none. Filters may recursively contain `and`, `or`, and a single `not` condition.

Expressions support `==`, `!=`, `>`, `>=`, `<`, and `<=`. Values may be strings, numbers, booleans, or `null`. Lists support `.contains(...)`, and `file.inFolder(...)` includes descendant folders.

Properties use a shorthand such as `status`, an explicit path such as `note.project.status`, or bracket access such as `note["project status"]`. File properties are `file.name`, `file.ext`, `file.path`, and `file.folder`.

String comparisons are case-sensitive. Missing properties compare equal to `null`, unequal to non-null values, and false for ordering and containment operations.

## Groups

Groups classify selected nodes after the graph filter has run. The first matching group wins, so group order matters.

Every group requires a unique `name`, a `filters` expression, and a non-empty `node` object. `node` accepts `color`, `size`, `border`, and `hover`. Group node fields override the corresponding regular node fields individually; omitted fields inherit from `display.node`.

## Orphan nodes

An orphan node has no Wiki Link edge to another selected node. `display.orphan.show` defaults to `true`; setting it to `false` removes orphan nodes from the Graph View.

When an orphan is displayed, `display.orphan.node` replaces both the regular node appearance and any matching group appearance.

## Node sizing

`display.node.size` and `display.orphan.node.size` are relative multipliers from `0.5` through `3.0`. `groups[].node.size` uses the same range and is an additional multiplier on the regular node size.

`display.node.propertional` defaults to `true`. When enabled, incoming Wiki Link count adds damped logarithmic growth capped at 4 times the base radius. The display and group size multipliers are applied to that derived radius. Larger nodes also receive proportionally stronger center gravity. Set `propertional: false` to disable link-derived growth for regular and grouped nodes.

## Styling

### Colors

Colors accept `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, and `oklch()`. Alpha is supported in every applicable form, including `#00000000`.

Omitted colors are resolved from the active application theme. Explicit YAML colors always take precedence.

### Node appearance

`display.node.color` styles regular nodes. `groups[].node.color` overrides it for the first matching group, while `display.orphan.node.color` overrides both for orphan nodes.

### Node borders

`display.node`, `groups[].node`, and `display.orphan.node` accept the same `border` fields. Group border fields override regular border fields individually. Border width ranges from `0.0` through `5.0`; zero explicitly hides the border. A border color without a width uses `1.0`. A border width without a color uses the node's resolved fill color.

### Edge appearance

`display.edge.color` and `display.edge.width` style edges. Edge width ranges from `0.5` through `5.0` and defaults to `1.0`.

### Node hover appearance

The `hover` fields under `display.node`, `groups[].node`, and `display.orphan.node` override the resolved normal appearance one field at a time. Group hover fields override regular hover fields individually. An omitted hover color uses the application theme's accent color. Omitted hover-border fields inherit the resolved normal border. If the first configured border is a hover border with only a color, its width is `1.0`.

`hover.size` is a multiplier from `0.5` through `3.0` and defaults to `1.0`. It applies after the node size, group size, and proportional incoming-link growth have been resolved.

### Edge hover appearance

`display.edge.hover.direction.outgoing`, `display.edge.hover.direction.incoming`, and `display.edge.hover.direction.both` independently style highlighted edges. Each color defaults to the application theme's accent color, and each width defaults to `display.edge.width`. Hover edge widths use the same `0.5` through `5.0` range as normal edge widths.

## Hover behavior

Hovering a node highlights the node, every incident edge, and every directly connected sibling node. A one-way link leaving the hovered node uses the `outgoing` style, and a one-way link entering it uses `incoming`. When both directed Wiki Links exist between the hovered node and the same sibling, both edges use `both`.

Every unrelated node and edge is dimmed. Edges beyond those directly incident to the hovered node are not highlighted. Hover focus is disabled while a node or the scene is being dragged.

## Arrowheads

`display.edge.arrow` defaults to `false`. Set it to `true` to draw directed arrowheads.

Arrowheads use the normal edge color. During hover focus, they use the outgoing, incoming, or reciprocal edge hover color corresponding to their link.

## Labels and zoom

Below `2.5×` zoom, only the hovered node's filename stem is displayed. It is rendered directly below the node without a bubble. For example, `Inbox/day.md` displays `day`.

At `2.5×` zoom and above, the filename stem is displayed below every node whose circle intersects the viewport. Off-screen nodes do not create labels. This high-zoom label mode remains active while dragging or panning because it depends on camera zoom rather than hover focus.

## Physics

The optional `physics` section configures the force simulation. Every omitted value uses its tuned default shown in the example above.

- `center.strength` pulls nodes toward the graph origin.
- `repulsion.strength` pushes nodes apart.
- `link.strength` controls how strongly linked nodes move toward their preferred distance.
- `link.distance` is that preferred distance.

Strengths must be finite, non-negative numbers; setting a strength to zero disables that force. Link distance must be finite and greater than zero. Acceleration and velocity safety limits remain internal and apply regardless of configured strengths.

## Limits

A Graph Definition has no default `limit`: it renders every matching node up to the hard safety ceiling of 50,000 nodes. An explicit `limit` between 1 and 50,000 lowers that bound. When a graph matches more nodes than its effective bound, Datalith renders the first nodes up to the bound and shows a banner reporting how many of the matching nodes were rendered.
