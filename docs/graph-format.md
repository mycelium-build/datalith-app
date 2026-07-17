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
  orphans:
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
  arrows:
    show: false
    color: '#ffffff80'

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

Frontmatter properties use a shorthand such as `status`, an explicit path such as `note.project.status`, or bracket access such as `note["project status"]`. File properties are `file.name`, `file.ext`, `file.path`, and `file.folder`.

String comparisons are case-sensitive. Missing properties compare equal to `null`, unequal to non-null values, and false for ordering and containment operations.

## Groups and colors

The first matching Graph Group wins. A group requires a unique `name`, `filters`, and at least one of `color` or `size`. Group sizes are relative multipliers from `0.5` through `3.0`.

`display.node.propertional` defaults to `true`. When enabled, a node grows on a damped logarithmic curve based on its incoming Wiki Link count. Link-derived growth is capped at 4 times the base radius so highly connected nodes remain prominent without dominating the graph. `display.node.size` and a matching group's `size` remain multipliers on that derived size. Larger nodes also receive proportionally stronger center gravity. Set `propertional: false` to keep link count from affecting node size.

Colors accept `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, and `oklch()`. Alpha is supported in every applicable form, including `#00000000`.

Orphan-node appearance overrides general and group node appearance. Arrowheads inherit the edge color when their own color is omitted.

## Borders and hover

`display.node` and `display.orphans.node` accept the same `border` and `hover` fields. Border widths range from `0.0` through `5.0`; zero explicitly hides the border. Supplying a border color without a width uses `1.0`, while supplying a width without a color uses the node's resolved fill color.

Node hover fields override the normal node appearance one field at a time. An omitted node hover color uses the application theme's accent color, and omitted hover-border fields inherit their normal-border counterparts. If a hover border is the first border to specify only a color, its width is `1.0`.

`hover.size` is a multiplier from `0.5` through `3.0`. It applies to the fully resolved radius after the node size, Graph Group size, and proportional incoming-link growth have been applied. It defaults to `1.0`.

`display.edge.hover.direction.outgoing`, `display.edge.hover.direction.incoming`, and `display.edge.hover.direction.both` independently control highlighted edge styles. Each optional color defaults to the application theme's accent color, and each optional width defaults to `display.edge.width`. Hover edge widths use the same `0.5` through `5.0` range as normal edge widths.

Hovering a node highlights every incident edge and directly connected sibling node. A one-way link leaving the hovered node uses `outgoing`, and a one-way link entering it uses `incoming`. When both directed Wiki Links exist between the hovered node and the same sibling, both edges use `both`. Arrowheads inherit the corresponding directional hover color. Every unrelated node and edge is dimmed, and edges beyond those directly incident to the hovered node are not highlighted. This focus treatment is disabled while any node or the scene is being dragged.

The hovered node's filename stem is rendered directly below the node without a bubble. Below the high-zoom threshold, no other node names are displayed.

At `2.5×` zoom and above, the filename stem is displayed below every node whose circle is visible in the viewport. Off-screen nodes do not create labels. This high-zoom label mode remains active while dragging or panning because it depends on camera zoom rather than hover focus.

## Physics

The optional `physics` section configures the force simulation. Every omitted value uses its tuned default shown in the example above.

- `center.strength` pulls nodes toward the graph origin.
- `repulsion.strength` pushes nodes apart.
- `link.strength` controls how strongly linked nodes move toward their preferred distance.
- `link.distance` is that preferred distance.

Strengths must be finite, non-negative numbers; setting a strength to zero disables that force. Link distance must be finite and greater than zero. Acceleration and velocity safety limits remain internal and apply regardless of configured strengths.

## Limits

The default result limit is 2,000 nodes and the hard safety ceiling is 10,000. Datalith reports an error instead of silently truncating a graph.
