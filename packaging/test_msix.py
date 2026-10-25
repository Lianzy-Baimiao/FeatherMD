import tempfile
import unittest
from pathlib import Path
import zipfile

import msix


class PackagingTests(unittest.TestCase):
    def test_version(self):
        for value in ("1.0.10.0", "2.1.0.0", "65535.65535.65535.0"):
            self.assertEqual(msix.version(value), value)
        for value in ("0.1.10.0", "1.0.0.1", "1.0.0", "01.0.0.0", "1.65536.0.0", "1.0.-1.0"):
            with self.assertRaises(ValueError):
                msix.version(value)

    def test_identity_is_explicit_and_never_a_test_submission(self):
        msix.validate_identity(msix.TEST_IDENTITY)
        for bad in ({}, msix.TEST_IDENTITY, {**msix.TEST_IDENTITY, "partner_center_verified": True}):
            with self.assertRaises(ValueError):
                msix.validate_identity(bad, production=True)
        with self.assertRaises(ValueError):
            msix.validate_identity({**msix.TEST_IDENTITY, "name": "../bad"})

    def test_manifest_contract(self):
        tree = msix.manifest(msix.TEST_IDENTITY, "1.0.10.0")
        ns = msix.NS
        self.assertEqual(tree.find("f:Identity", ns).get("ProcessorArchitecture"), "x64")
        app = tree.find("f:Applications/f:Application", ns)
        self.assertEqual(app.get("Id"), "FeatherMD")
        self.assertEqual(app.get("Executable"), "FeatherMD.exe")
        self.assertEqual(app.get("{" + ns["uap10"] + "}TrustLevel"), "mediumIL")
        self.assertEqual([x.get("Name") for x in tree.find("f:Capabilities", ns)], ["runFullTrust"])
        # Markdown, then plain text and logs: Open With entries, never a default.
        associations = tree.findall(".//uap3:FileTypeAssociation", ns)
        self.assertEqual([a.get("Name") for a in associations], ["feathermd.markdown", "feathermd.plaintext"])
        for association in associations:
            self.assertEqual(association.get("Parameters"), '"%1"')
            self.assertEqual(association.get("MultiSelectModel"), "Single")
        self.assertEqual([[x.text for x in a.findall("uap:SupportedFileTypes/uap:FileType", ns)] for a in associations],
                         [[".md", ".markdown", ".mdown", ".mkd"], [".txt", ".text", ".log"]])
        self.assertEqual(len(tree.findall("f:Resources/f:Resource", ns)), 4)

    def test_archive_rejects_leaks_and_modified_files(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            layout = root / "layout"
            layout.mkdir()
            (layout / "FeatherMD.exe").write_bytes(b"test")
            archive = root / "test.msix"
            with zipfile.ZipFile(archive, "w") as z:
                z.write(layout / "FeatherMD.exe", "FeatherMD.exe")
            msix.verify_archive(archive, layout)
            with zipfile.ZipFile(archive, "a") as z:
                z.writestr("FeatherMD.ini", "private")
            with self.assertRaises(ValueError):
                msix.verify_archive(archive, layout)

    def test_non_pe_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "bad.exe"
            path.write_bytes(b"not an executable")
            with self.assertRaises(ValueError):
                msix.verify_binary(path)


if __name__ == "__main__":
    unittest.main()
