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

The launcher temporarily stamps `CHANNEL` as Stable for the build and restores
it before starting the app. Avoid building another channel at the same time.

Choose **Datalith → Check for updates**, directly below **About Datalith**, or
wait about ten seconds for the automatic check. Expect download progress for
about eight seconds, followed by **Restart to update**.

- Ordinary Quit discards the update without applying it.
- **Restart to update** quits and runs the fake update. Within a few seconds,
  `target/update-ui/applied` should contain `Fake update applied`. Datalith does
  not reopen. The fake update only changes files under `target/update-ui`; on
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

## Linux package mode

On Linux, launch `sh target/update-ui/run-package.sh` instead of `run.sh` to
test package behavior. The update control should open the website download
page; the server should receive no `/bundle` request and no `applied` marker
should be created. Use a freshly started server to clear previous results.

## Packaged release verification

Run on both x86_64 and ARM64 for Linux, macOS, and Windows, using two
updater-bearing versions. The fake server tests UI and handoff; it does not
replace these checks with real packages and the published manifest.

1. Fresh-install the older AppImage, macOS app from its DMG, or Windows
   current-user NSIS package. Launch it and check the version in About.
2. Discover the newer release and wait for **Restart to update**. Keep using
   the app during download; it should remain responsive.
3. Quit normally. The installed version must remain unchanged. Relaunch;
   the update must download again.
4. Choose **Restart to update**. Datalith should quit, install, and reopen
   with the newer About version. Check that settings and a test Vault survive.
5. Uninstall using the platform's normal method. On Windows, check that
   updating did not duplicate shortcuts or the uninstall entry.

For `.deb`, `.rpm`, and `.pkg.tar.zst`, install through the matching package
manager and check for an update. The control must open
`https://mycelium-build.github.io/datalith/#download`, without downloading or
replacing application files. Compare package verification results before and
after (`dpkg --verify datalith`, `rpm -V datalith`, or `pacman -Qkk datalith`).

Record the package versions, OS/architecture, result, and any Gatekeeper,
SmartScreen, or other unsigned-application warnings for each run.

For release removal, use a disposable repository configured with the same
release and Pages workflows. Publish two signed stable fixture releases,
confirm the manifest offers the newer one, then delete it through GitHub's UI.
The regeneration workflow must succeed and the live manifest must offer the
older release. Delete that release too; the manifest should offer no update.

## Stable and Preview side by side

Install a Stable and Preview package on each platform. Both should appear with
separate names and launcher icons; Preview's icon and in-app monolith are yellow.
Launch both, change a setting in one, and confirm the other keeps its own value.
Both can open the same test Vault, with separate catalog and search caches. Uninstall Preview and confirm Stable still
launches with its settings and shortcuts intact.

For a local Preview UI check, put `preview` in `CHANNEL` and run
`cargo run --locked --bin datalith`. Expect “Datalith Preview” in the menu and
About, with the full stamped RC version when built with
`DATALITH_RELEASE_TAG=vX.Y.Z-rc.N`, and separate settings. Put `dev` back in `CHANNEL` when finished; a normal `cargo run` should show
a green monolith and the current Git commit in About.
Preview update discovery needs the D7 manifest; a manual check currently fails
until that endpoint exists.
