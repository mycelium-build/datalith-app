---
category: format
---

A **Graph View** renders matching files as nodes and their wiki links as edges, arranged by a force simulation. It is one view type of a Base, declared with `type: graph`, and shares filtering and limits with every other view:

```yaml
views:
  - type: graph
    name: Link map
    limit: 2000

    filters:
      and:
        - 'file.inFolder("Inbox")'
        - 'status != "archived"'
        - 'priority >= 3'

    classes:
      - name: Done
        filters: 'status == "done"'
        node:
          color: '#ff0000cc'
          size: 1.25

    display:
      legend: true
      node:
        color: '#7c8cff'
        size: 1.0
        proportional: true
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

    physics:
      center:
        strength: 0.002
      repulsion:
        strength: 1024.0
      link:
        strength: 0.04
        distance: 128.0
```

# Filters

Graph views use the full [[formats/bases/Overview|Base expression language]]: any function, comparison, arithmetic, or boolean combination that other views support also works here. The database evaluates the filter directly — a graph renders exactly the rows the query returns.

Omitting `filters` selects every file in the Vault.

# Classes

Classes classify selected nodes after the filter has run. The **first** class whose filter matches wins, so class order matters.

Every class requires a unique `name`, a `filters` expression, and a non-empty `node` object. `node` accepts `color`, `size`, `border`, and `hover`. Class node fields override the corresponding regular node fields individually; omitted fields inherit from `display.node`.

Unlike `groupBy`, classes do not partition rows into sections — they are conditional styles for a link map.

# Legend

Graph views with at least one class show a legend overlay in the top-right corner: one color dot and class name per class, in declaration order. A class without its own `color` shows the resolved `display.node.color`, or the application theme color when both are unset.

`display.legend` defaults to `true`; set it to `false` to hide the legend. Views without classes never show one.

# Summaries

When the view declares `summaries`, a box appears under the legend in the top-right corner with one `Pages Sum: 350` line per summary. Graph views without summaries show nothing.

# Orphan nodes

An orphan node has no edge to another selected node. `display.orphan.show` defaults to `true`; setting it to `false` removes orphan nodes from the Graph View.

When an orphan is displayed, `display.orphan.node` replaces both the regular node appearance and any matching class appearance.

# Node sizing

`display.node.size` and `display.orphan.node.size` are relative multipliers from `0.5` through `3.0`. `classes[].node.size` uses the same range and is an additional multiplier on the regular node size.

`display.node.proportional` defaults to `true`. When enabled, incoming link count adds damped logarithmic growth capped at 4 times the base radius. The display and class size multipliers are applied to that derived radius. Larger nodes also receive proportionally stronger center gravity. Set `proportional: false` to disable link-derived growth for regular and classified nodes.

# Styling

## Colors

Colors accept `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, and `oklch()`. Alpha is supported in every applicable form, including `#00000000`.

Omitted colors are resolved from the active application theme. Explicit YAML colors always take precedence.

## Node appearance

`display.node.color` styles regular nodes. `classes[].node.color` overrides it for the first matching class, while `display.orphan.node.color` overrides both for orphan nodes.

## Node borders

`display.node`, `classes[].node`, and `display.orphan.node` accept the same `border` fields. Class border fields override regular border fields individually. Border width ranges from `0.0` through `5.0`; zero explicitly hides the border. A border color without a width uses `1.0`. A border width without a color uses the node's resolved fill color.

## Edge appearance

`display.edge.color` and `display.edge.width` style edges. Edge width ranges from `0.5` through `5.0` and defaults to `1.0`. Edges are derived from each rendered note's resolved outgoing links; only edges between two rendered nodes are drawn.

## Node hover appearance

The `hover` fields under `display.node`, `classes[].node`, and `display.orphan.node` override the resolved normal appearance one field at a time. An omitted hover color uses the application theme's accent color. Omitted hover-border fields inherit the resolved normal border. If the first configured border is a hover border with only a color, its width is `1.0`.

`hover.size` is a multiplier from `0.5` through `3.0` and defaults to `1.0`. It applies after the node size, class size, and proportional incoming-link growth have been resolved.

## Edge hover appearance

`display.edge.hover.direction.outgoing`, `display.edge.hover.direction.incoming`, and `display.edge.hover.direction.both` independently style highlighted edges. Each color defaults to the application theme's accent color, and each width defaults to `display.edge.width`. Hover edge widths use the same `0.5` through `5.0` range as normal edge widths.

# Hover behavior

Hovering a node highlights the node, every incident edge, and every directly connected sibling node. A one-way link leaving the hovered node uses the `outgoing` style, and a one-way link entering it uses `incoming`. When both directed links exist between the hovered node and the same sibling, both edges use `both`.

Every unrelated node and edge is dimmed. Hover focus is disabled while a node or the scene is being dragged.

Clicking a node opens the underlying file; hold ⌘ (or Ctrl elsewhere) while clicking to open it in a new tab.

# Arrowheads

`display.edge.arrow` defaults to `false`. Set it to `true` to draw directed arrowheads.

Arrowheads use the normal edge color. During hover focus, they use the outgoing, incoming, or reciprocal edge hover color corresponding to their link.

# Labels and zoom

Below `2.5×` zoom, only the hovered node's filename stem is displayed. It is rendered directly below the node without a bubble. For example, `Inbox/day.md` displays `day`.

At `2.5×` zoom and above, the filename stem is displayed below every node whose circle intersects the viewport. Off-screen nodes do not create labels.

# Physics

The optional `physics` section configures the force simulation. Every omitted value uses its tuned default shown in the example above.

- `center.strength` pulls nodes toward the graph origin.
- `repulsion.strength` pushes nodes apart.
- `link.strength` controls how strongly linked nodes move toward their preferred distance.
- `link.distance` is that preferred distance.

Strengths must be finite, non-negative numbers; setting a strength to zero disables that force. Link distance must be finite and greater than zero. Acceleration and velocity safety limits remain internal and apply regardless of configured strengths.

# Limits

`limit` behaves exactly as in every other Base view: between 1 and 50,000, defaulting to the 50,000-row safety ceiling, with omitted files reported by the view.
