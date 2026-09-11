import base64
import copy
from pathlib import Path
import subprocess
import tempfile
import unittest

from generate_manifest import bundles, generate_manifest, latest_stable


class ManifestTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.directory = Path(cls.temporary.name)
        cls.secret = cls.directory / "secret.key"
        raw_key = cls.directory / "public.key"
        subprocess.run(["minisign", "-G", "-W", "-s", str(cls.secret), "-p", str(raw_key)],
                       check=True, capture_output=True)
        cls.public_key = cls.directory / "public-key.txt"
        cls.public_key.write_text(base64.b64encode(raw_key.read_bytes()).decode())
        cls.release = {
            "tag_name": "v0.2.0", "draft": False, "prerelease": False,
            "published_at": "2026-09-11T12:00:00Z", "assets": [],
        }
        for name, _ in bundles("0.2.0").values():
            bundle = cls.directory / name
            bundle.write_bytes(f"Fixture for {name}".encode())
            raw_signature = cls.directory / "signature"
            subprocess.run(["minisign", "-S", "-s", str(cls.secret), "-m", str(bundle),
                            "-x", str(raw_signature), "-t", f"timestamp:1\tfile:{name}"],
                           check=True, capture_output=True)
            signature = cls.directory / f"{name}.sig"
            signature.write_text(base64.b64encode(raw_signature.read_bytes()).decode())
            for asset in (bundle, signature):
                cls.release["assets"].append({
                    "name": asset.name, "size": asset.stat().st_size,
                    "browser_download_url": f"https://github.com/example/app/releases/download/v0.2.0/{asset.name}",
                })

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def generate(self, release=None):
        return generate_manifest(release or self.release, self.directory, self.public_key)

    def test_signed_six_platform_manifest(self):
        manifest = self.generate()
        self.assertEqual(manifest["version"], "0.2.0")
        self.assertEqual(manifest["pub_date"], self.release["published_at"])
        self.assertEqual({key: value["format"] for key, value in manifest["platforms"].items()}, {
            "linux-x86_64": "appimage", "linux-aarch64": "appimage",
            "macos-x86_64": "app", "macos-aarch64": "app",
            "windows-x86_64": "nsis", "windows-aarch64": "nsis",
        })

    def test_signed_preview_bundles_use_the_full_rc_version(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            release = dict(self.release, tag_name="v0.2.0-rc.3", prerelease=True, assets=[])
            for name, _ in bundles("0.2.0-rc.3").values():
                self.assertTrue(name.startswith("datalith-preview_0.2.0-rc.3_"))
                bundle = directory / name
                bundle.write_bytes(f"Preview fixture {name}".encode())
                raw = directory / "signature"
                subprocess.run(["minisign", "-S", "-s", str(self.secret), "-m", str(bundle),
                                "-x", str(raw), "-t", f"timestamp:1\tfile:{name}"],
                               check=True, capture_output=True)
                signature = directory / f"{name}.sig"
                signature.write_text(base64.b64encode(raw.read_bytes()).decode())
                for asset in (bundle, signature):
                    release["assets"].append(dict(name=asset.name, size=asset.stat().st_size,
                                                 browser_download_url=f"https://example.com/{asset.name}"))
            manifest = generate_manifest(release, directory, self.public_key)
            self.assertEqual(manifest["version"], "0.2.0-rc.3")
            self.assertEqual(len(manifest["platforms"]), 6)
            self.assertIsNone(latest_stable([release]))

    def test_missing_bundle_or_signature(self):
        for index in range(12):
            with self.subTest(index=index):
                release = copy.deepcopy(self.release)
                release["assets"].pop(index)
                with self.assertRaises(ValueError):
                    self.generate(release)

    def test_duplicate(self):
        release = copy.deepcopy(self.release)
        release["assets"].append(release["assets"][0])
        with self.assertRaises(ValueError):
            self.generate(release)

    def test_wrong_version_or_target(self):
        for replacement in ("0.3.0", "i686"):
            release = copy.deepcopy(self.release)
            release["assets"][0]["name"] = release["assets"][0]["name"].replace(
                "0.2.0" if replacement == "0.3.0" else "x86_64", replacement)
            with self.assertRaises(ValueError):
                self.generate(release)

    def test_unexpected_update_asset(self):
        release = copy.deepcopy(self.release)
        release["assets"].append({"name": "unrecognized.AppImage"})
        with self.assertRaises(ValueError):
            self.generate(release)

    def test_tampered_bundle(self):
        bundle = self.directory / self.release["assets"][0]["name"]
        original = bundle.read_bytes()
        try:
            bundle.write_bytes(bytes([original[0] ^ 1]) + original[1:])
            with self.assertRaises(subprocess.CalledProcessError):
                self.generate()
        finally:
            bundle.write_bytes(original)

    def test_signature_cannot_move_between_platforms(self):
        signatures = [self.directory / asset["name"] for asset in self.release["assets"]
                      if asset["name"].endswith(".sig")]
        original = signatures[0].read_bytes()
        try:
            signatures[0].write_bytes(signatures[1].read_bytes())
            with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                self.generate()
        finally:
            signatures[0].write_bytes(original)

    def test_wrong_key(self):
        key = self.directory / "wrong.pub"
        subprocess.run(["minisign", "-G", "-W", "-s", str(self.directory / "wrong.key"),
                        "-p", str(key)], check=True, capture_output=True)
        encoded_key = self.directory / "wrong-key.txt"
        encoded_key.write_text(base64.b64encode(key.read_bytes()).decode())
        with self.assertRaises(subprocess.CalledProcessError):
            generate_manifest(self.release, self.directory, encoded_key)

    def test_size_mismatch(self):
        release = copy.deepcopy(self.release)
        release["assets"][0]["size"] += 1
        with self.assertRaises(ValueError):
            self.generate(release)

    def test_selects_highest_semver_and_handles_removal(self):
        def release(tag, **changes):
            return dict(self.release, tag_name=tag, **changes)
        older = release("v0.2.9")
        newer = release("v0.2.10")
        ignored = [release("v0.1.0"), release("v0.3.0-rc.1", prerelease=True),
                   release("v0.4.0", draft=True), release("v0.5.0", prerelease=True)]
        self.assertIs(latest_stable([newer, *ignored, older]), newer)
        self.assertIs(latest_stable([*ignored, older]), older)
        self.assertIsNone(latest_stable(ignored))


if __name__ == "__main__":
    unittest.main()
