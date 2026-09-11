# Testing auto-update

From the repository root, start the fake server on Linux, macOS, or Windows:

```sh
cargo run --locked --example update-server
```

Keep it running. In another terminal, launch the app with the generated script:

| Platform | Command |
| --- | --- |
| Linux / macOS | `sh target/update-ui/run.sh` |
| Windows (PowerShell) | `powershell -ExecutionPolicy Bypass -File target/update-ui/run.ps1` |

Choose **Datalith → Check for updates**, directly below **About Datalith**, or
wait about ten seconds for the automatic check. Expect download progress for
about eight seconds, followed by **Restart to update**.

- Ordinary Quit discards the update without applying it.
- **Restart to update** quits and runs the fake update. Within a few seconds,
  `target/update-ui/applied` should contain `Fake update applied`. Datalith does
  not reopen. The fixture only changes files under `target/update-ui`; on
  Windows it simulates the installer handoff without installing anything.
- Turn off **Settings → General → Updates → Automatically update Datalith** and
  confirm the menu command still works. Restore the preference afterward.
- Check keyboard activation, light/dark themes, larger text, and a narrow sidebar.

To test another scenario, edit `target/update-ui/scenario` to one of these values:

| Value | Expected behavior |
| --- | --- |
| `ready` | Download progress, then Restart to update |
| `unknown-length` | Indeterminate progress, then Restart to update |
| `up-to-date` | “Datalith is up to date” notification |
| `check-error` | Check failure notification |
| `download-error` | Download failure notification |
| `bad-signature` | Verification fails; no update is staged |

Restart the app with the generated script between scenarios and use the menu
command. Automatic failures are silent. Restarting the server clears the marker
and regenerates the test key and launch script.

When finished, stop the app and server. Run `cargo run --locked --bin datalith`
normally to return to a development build without update controls.
