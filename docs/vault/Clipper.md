---
category: reference
---

The **Clipper** is the Datalith browser extension (Chrome and Firefox) that saves web pages as Markdown notes into your vaults. It talks to the **[[Local Server]]** embedded in the app.

# Getting started

1. Install the extension in your browser (see the Clipper's README for the download and load-unpacked steps).
2. In its options, under **General → Connection**, check the URL and port match Datalith's **Settings → Local Server**, paste the token if you use one, and click **Sync**.
3. Click the toolbar icon on any page to open the Panel and save your first note.

Keyboard shortcuts: **Alt+Shift+O** open the Clipper, **Alt+Shift+H** toggle the highlighter, **Alt+Shift+R** toggle the reader.

# Panel

The Panel is a full-height sidebar to review the clip before saving. The page title, author, description and word count are prefilled for you. You can edit the name, properties and content, with a live Markdown preview, and pick the vault and folder. Saving writes the note into the vault, the folder is created on first save, colliding names get a number, and the note appears in Datalith without any reload. A link then offers to **Open in Datalith** and reveal the saved note.

# Templates

Templates are named recipes that fill the note for you using the [Knap](https://knap.md) engine: variables (title, author, selection, highlights, image…), CSS-selector extraction (`{{selector:h1}}`), filters, conditionals and loops. A URL glob trigger can automatically select the appropriate Template for each site. For more information about editing Templates, go to extension options, where a reference bar lists everything available and live syntax checking helps as you type.

# Interpreter

The Interpreter adds optional LLM support: `{{"prompt"}}` expressions in a Template are sent to your own OpenAI-compatible provider and replaced with the model's answer. For example to summarize a page. While the Interpreter is off, prompts show as plain text. It is off by default, and the browser permission for a provider's address is only requested when you add it.

# Highlighter

Turn on the highlighter and paint text directly on the page. Everything you highlight is collected and lands in the saved note, a simple way to gather quotes while you read.

# Reader view

The reader view is a distraction-free overlay on the page, with configurable font, size, line height and link coloring. Read first, clip what matters.

# Themes

The Clipper comes in light, dark or auto, matching Datalith's own themes.

# Vaults

Only vaults you have opened in Datalith (your last used and recent vaults) are offered to the Clipper. **Sync** refreshes the list after you open a new vault. If the app was closed, the Clipper launches it for you the next time you save or open a note.
