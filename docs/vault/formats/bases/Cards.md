---
category: format
---

A **Cards View** renders matching files as a responsive, virtualized image grid. Declare it with `type: cards`:

```yaml
views:
  - type: cards
    name: Gallery
    order:
      - file.name
      - cover
      - author
    image: cover
    imageFit: cover
    imageAspectRatio: 1.5
    cardSize: 220
```

# Cards Settings

- `image`: note or file property used for the card image. Local wikilinks, local paths, and HTTP(S) URLs are supported.
- `imageFit`: `cover` or `contain`. The default is `cover`.
- `imageAspectRatio`: positive numeric image aspect ratio. The default is `1`.
- `cardSize`: target card width in pixels. The default is `200`.

The configured `order` properties are shown below the image. `file.name` is an interactive link to the file. Press and hold a card image to show it fullscreen; releasing the mouse returns to the cards view.

All views are read-only. Link cells navigate to files; edit note properties in the Markdown editor.

Shared syntax — filters, ordering, grouping, summaries, and limits — is documented in [[formats/bases/Overview|the Base overview]].
