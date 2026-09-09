---
category: reference
---

The **Local Server** is a small HTTP server embedded in Datalith, reachable only from your own machine (`127.0.0.1`). It is how other programs talk to the app: the **[[Clipper]]** browser extension is its first client, and any local tool can use the same API. The server documents itself with a generated OpenAPI spec at `/openapi.json`, browsable at `http://127.0.0.1:42908/docs` while it runs.

# Setup

1. Enable the server in **Settings → Local Server**. The status line shows the address, e.g. `Running on 127.0.0.1:42908`.
2. Point your client at the same address.
3. (Optional) Click **Generate** to create a bearer token, then **Copy** and paste it into the client. When a token is set, clients must send it to use the server, when it is empty, no authentication is used.

Changing the port takes effect immediately. If the port is already taken, a notification explains what happened.

# API

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/ping` | Health check, returns the app name and version. |
| `GET` | `/api/vaults` | Lists your last used and recent vaults (name and path). |
| `POST` | `/api/notes` | Saves a note, returns the vault-relative path it was written to. |
| `GET` | `/openapi.json` | The generated OpenAPI spec describing the API. |
| `GET` | `/docs` | Browsable documentation for the API. |

A save sends `vault`, `name`, optional `folder`, `properties`, and `content`:

```json
{
    "vault": "Second Brain",
    "name": "My Clip",
    "folder": "Clips",
    "properties": { "source": "https://example.com" },
    "content": "Note body in Markdown."
}
```

The folder is created on first save, and colliding names get a number: `Note.md`, `Note 1.md`, `Note 2.md`. The note is written as a Markdown file with YAML frontmatter for its properties, exactly like a note you would create yourself, the Vault database picks it up automatically.

# Security

The server binds to `127.0.0.1` only, nothing on your network can reach it. The API is open to every app on your machine. Webpages in your browser are the one exception: they are refused, so a site you visit cannot list your vaults or plant notes. If you want extra security you set a token.

# Deep links

Datalith also registers the `datalith://` scheme, which works even when the app is closed:

- `datalith://launch` opens the app.
- `datalith://open?vault=Notes&path=Clips/My%20Note.md` opens a vault-relative note in the named vault, matched by name or full path, switching to it if needed.
