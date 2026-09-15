---
status: accepted
---

# Use a local HTTP server for program integration, keeping deep links for launch and reveal

The Web clipper needs to exchange data with a running Datalith: discover vaults, save notes, and learn where a note landed. Datalith already registers the `datalith://` scheme, which the operating system routes to the app to launch it or open a note, so that channel could in principle carry the whole integration.

We use a small HTTP server bound to `127.0.0.1` as the data channel for program integration, and keep Deep links for what they are good at: launching the app and revealing a note. The server offers request/response endpoints (`GET /ping`, `GET /api/vaults`, `POST /api/notes`) documented by a generated OpenAPI spec, guarded by the loopback bind, an extension-only preflight that refuses plain webpages, and an optional bearer Server token.

A Deep link is a one-way command into the app with no response channel. A browser extension that navigates to `datalith://save?...` gets no vault list back, no saved path, and no error message. HTTP gives request/response semantics with off-the-shelf machinery: JSON parsing, content-type enforcement, body limits, CORS, bearer auth. Saving should not interrupt the user's workflow. Opening a deep link brings Datalith to the foreground, whereas the local server can save the note in the background while the user continues working in the browser. And any local program can integrate against the documented API with plain HTTP with no per-platform scheme plumbing. The cost is a listening socket.

## Considered options

- **Carry the whole integration over deep links:** keeps a single mechanism and opens no port, but offers no response channel for browser clients, forces note content through URL query strings, foregrounds the app on every save, and grows a bespoke URL grammar for every new operation.

## Consequences

- Integration has two surfaces with clear roles: the Local Server for data exchange, Deep links for launch and reveal.
- Any local program can integrate against `/openapi.json` without our involvement.
- A listening socket is a broader attack surface than a scheme handler, so the server must keep its guardrails: loopback bind, extension-only preflight, optional bearer token.
- The API works only while Datalith is running. If the server is not available, clients receive a connection refused error. When needed, the external program must first open Datalith with a launch deep link before using the API.
