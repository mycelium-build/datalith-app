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
        for tag in ("1.2.3", "v1.2.3-beta.1", "v1.2.3-rcfoo"):
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
                                update_endpoint=f"https://mycelium-build.github.io/datalith-app/updates/{channel}.json",
                                version=version)
                with patch.object(release.subprocess, "check_output", return_value=json.dumps(identity)):
                    release.prepare(f"v{version}", temporary)
                config = json.loads((directory / "packager.json").read_text())
                self.assertEqual(config["version"], version)
                self.assertEqual(config["binaries"][0]["path"], stem)
                self.assertEqual((directory / f"{stem}{suffix}").read_bytes(), b"binary")
                self.assertTrue(all(Path(icon).is_file() for icon in config["icons"]))
                for key in identity:
                    with patch.object(release.subprocess, "check_output",
                                      return_value=json.dumps(dict(identity, **{key: "wrong"}))):
                        with self.assertRaises(ValueError):
                            release.prepare(f"v{version}", temporary)


if __name__ == "__main__":
    unittest.main()
