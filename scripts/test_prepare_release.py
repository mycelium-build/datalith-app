import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("prepare_release", Path(__file__).with_name("prepare-release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseIdentityTests(unittest.TestCase):
    def test_release_tags_select_channel(self):
        self.assertEqual(release.channel_for_tag("v1.2.3"), "stable")
        self.assertEqual(release.channel_for_tag("v1.2.3-rc.2"), "preview")
        for tag in ("1.2.3", "v1.2.3-beta.1", "v1.2.3-rcfoo", "v1.2.3-rc", "v1.2.3-rc.01"):
            with self.assertRaises(ValueError):
                release.channel_for_tag(tag)

    def test_prepares_both_products_and_rejects_identity_drift(self):
        for channel, stem, product, version in (
            ("stable", "datalith", "Datalith", "1.2.3"),
            ("preview", "datalith-preview", "Datalith Preview", "1.2.3-rc.2"),
        ):
            with self.subTest(channel=channel), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary) / "release"
                directory.mkdir()
                suffix = ".exe" if release.os.name == "nt" else ""
                (directory / f"datalith{suffix}").write_bytes(b"binary")
                identity = dict(channel=channel, stem=stem, product_name=product,
                                identifier=f"build.mycelium.{stem}", data_directory=stem,
                                update_endpoint=f"https://updates.datalith.work/{channel}",
                                version=version)
                with patch.object(release.subprocess, "check_output", return_value=json.dumps(identity)):
                    release.prepare(f"v{version}", temporary)
                config = json.loads((directory / "packager.json").read_text())
                self.assertEqual(config["version"], version)
                self.assertEqual(config["binaries"][0]["path"], stem)
                self.assertEqual((directory / f"{stem}{suffix}").read_bytes(), b"binary")
                self.assertTrue(all(Path(icon).is_file() for icon in config["icons"]))
                background = Path(config["dmg"]["background"])
                self.assertTrue(background.is_absolute() and background.is_file())
                self.assertEqual(background.name, f"background-{channel}.png")
                for key in identity:
                    with patch.object(release.subprocess, "check_output",
                                      return_value=json.dumps(dict(identity, **{key: "wrong"}))):
                        with self.assertRaises(ValueError):
                            release.prepare(f"v{version}", temporary)

    def test_macos_signing_identity_applies_only_to_apple_targets(self):
        identity_name = "Developer ID Application: Example (TEAMID1234)"
        for triple, expected in (("aarch64-apple-darwin", identity_name),
                                 ("x86_64-apple-darwin", identity_name),
                                 ("x86_64-unknown-linux-gnu", None),
                                 ("x86_64-pc-windows-msvc", None)):
            with self.subTest(target=triple), tempfile.TemporaryDirectory() as temporary:
                # An absolute target replaces the "target" prefix, as in the test above.
                target = str(Path(temporary) / triple)
                directory = Path(target) / "release"
                directory.mkdir(parents=True)
                suffix = ".exe" if release.os.name == "nt" else ""
                (directory / f"datalith{suffix}").write_bytes(b"binary")
                identity = dict(channel="stable", stem="datalith", product_name="Datalith",
                                identifier="build.mycelium.datalith", data_directory="datalith",
                                update_endpoint="https://updates.datalith.work/stable",
                                version="1.2.3")
                with patch.object(release.subprocess, "check_output", return_value=json.dumps(identity)), \
                        patch.dict(release.os.environ, {"APPLE_SIGNING_IDENTITY": identity_name}):
                    release.prepare("v1.2.3", target)
                config = json.loads((directory / "packager.json").read_text())
                self.assertEqual(config.get("macos", {}).get("signing-identity"), expected)

    def test_liquid_glass_icon_applies_only_to_apple_targets(self):
        for triple, apple in (("aarch64-apple-darwin", True), ("x86_64-unknown-linux-gnu", False)):
            with self.subTest(target=triple), tempfile.TemporaryDirectory() as temporary:
                target = str(Path(temporary) / triple)
                directory = Path(target) / "release"
                directory.mkdir(parents=True)
                suffix = ".exe" if release.os.name == "nt" else ""
                (directory / f"datalith{suffix}").write_bytes(b"binary")
                identity = dict(channel="preview", stem="datalith-preview", product_name="Datalith Preview",
                                identifier="build.mycelium.datalith-preview", data_directory="datalith-preview",
                                update_endpoint="https://updates.datalith.work/preview",
                                version="1.2.3-rc.1")
                with patch.object(release.subprocess, "check_output", return_value=json.dumps(identity)), \
                        patch.dict(release.os.environ, {"APPLE_SIGNING_IDENTITY": ""}):
                    release.prepare("v1.2.3-rc.1", target)
                config = json.loads((directory / "packager.json").read_text())
                cars = [r for r in config["resources"] if r["target"] == "Assets.car"]
                plist = config.get("macos", {}).get("info-plist-path")
                if apple:
                    self.assertEqual(len(cars), 1)
                    self.assertTrue(Path(cars[0]["src"]).is_file())
                    self.assertEqual(Path(cars[0]["src"]).parent.name, "preview")
                    self.assertTrue(Path(plist).is_file())
                else:
                    self.assertEqual(cars, [])
                    self.assertIsNone(plist)


if __name__ == "__main__":
    unittest.main()
