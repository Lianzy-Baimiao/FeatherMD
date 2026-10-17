"""Build an unsigned MSIX from a static-CRT Store binary. No signing/installing.

Requires Pillow only for build-time icon resources, not at runtime.
Production identity JSON must come from FeatherMD's Partner Center identity page.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
NS = {"f": "http://schemas.microsoft.com/appx/manifest/foundation/windows10",
      "uap": "http://schemas.microsoft.com/appx/manifest/uap/windows10",
      "uap3": "http://schemas.microsoft.com/appx/manifest/uap/windows10/3",
      "uap10": "http://schemas.microsoft.com/appx/manifest/uap/windows10/10",
      "rescap": "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"}
for prefix, namespace in NS.items():
    ET.register_namespace("" if prefix == "f" else prefix, namespace)

TEST_IDENTITY = {"name": "FeatherMD.StoreTest", "publisher": "CN=FeatherMD Store Test",
                 "publisher_display_name": "FeatherMD Test", "display_name": "FeatherMD (Test)"}
SIZES = {"StoreLogo": 50, "Square44x44Logo": 44, "Square150x150Logo": 150}
FILES = ["LICENSE", "THIRD_PARTY_NOTICES.md", "docs/PRIVACY.md"]


def version(value):
    if not re.fullmatch(r"[1-9][0-9]*\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.0", value):
        raise ValueError("MSIX version must have four integers, first > 0 and fourth = 0")
    if any(int(n) > 65535 for n in value.split(".")):
        raise ValueError("MSIX version component exceeds 65535")
    return value


def validate_identity(identity, production=False):
    for key in TEST_IDENTITY:
        value = identity.get(key, "")
        if not isinstance(value, str) or not value.strip() or any(ord(c) < 32 for c in value):
            raise ValueError(f"Missing/invalid identity field: {key}")
        if len(value) > 256 or "REQUIRED" in value or "TODO" in value:
            raise ValueError(f"Unresolved identity field: {key}")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9.-]{2,49}", identity["name"]):
        raise ValueError("Invalid package identity name")
    if not identity["publisher"].startswith("CN="):
        raise ValueError("Publisher must be the exact Partner Center distinguished name")
    if production and (identity["name"] == TEST_IDENTITY["name"] or
                       identity["publisher"] == TEST_IDENTITY["publisher"] or
                       identity.get("partner_center_verified") is not True):
        raise ValueError("Production packaging requires verified FeatherMD Partner Center identity")
    return identity


def verify_binary(path):
    data = path.read_bytes()
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError("Expected a Windows PE executable")
    pe = struct.unpack_from("<I", data, 60)[0]
    if data[pe:pe+6] != b"PE\0\0\x64\x86":
        raise ValueError("Only x64 PE executables are supported")
    if b"FEATHERMD_STORE_CHANNEL_V1" not in data:
        raise ValueError("Build with --features store; refusing a portable binary")
    # Inspect the PE import table rather than searching arbitrary binary strings.
    optional = pe + 24
    if struct.unpack_from("<H", data, optional)[0] != 0x20B:
        raise ValueError("Expected PE32+")
    sections = optional + struct.unpack_from("<H", data, pe + 20)[0]
    count = struct.unpack_from("<H", data, pe + 6)[0]
    if count > 96:
        raise ValueError("Invalid PE sections")

    def offset(rva):
        for index in range(count):
            _, address, size, raw = struct.unpack_from("<IIII", data, sections + index * 40 + 8)
            if address <= rva < address + size and raw + rva - address < len(data):
                return raw + rva - address
        raise ValueError("Invalid PE import RVA")

    rva, size = struct.unpack_from("<II", data, optional + 120)
    imports = []
    for index in range(min(size // 20, 4096)):
        descriptor = struct.unpack_from("<IIIII", data, offset(rva + index * 20))
        if not any(descriptor):
            break
        pos = offset(descriptor[3])
        end = data.find(b"\0", pos, pos + 512)
        if end < 0:
            raise ValueError("Invalid PE import name")
        name = data[pos:end].decode("ascii").lower()
        if name.startswith(("vcruntime", "msvcp", "libgcc", "libstdc++", "libwinpthread")):
            raise ValueError(f"Non-static runtime: {name}")
        imports.append(name)
    else:
        raise ValueError("Missing import table terminator")
    return sorted(set(imports))


def manifest(identity, package_version):
    validate_identity(identity)
    tree = ET.parse(ROOT / "packaging/msix/AppxManifest.xml")
    tree.find("f:Identity", NS).attrib.update(Name=identity["name"], Publisher=identity["publisher"],
                                            Version=version(package_version))
    tree.find("f:Properties/f:DisplayName", NS).text = identity["display_name"]
    tree.find("f:Properties/f:PublisherDisplayName", NS).text = identity["publisher_display_name"]
    tree.find("f:Applications/f:Application/uap:VisualElements", NS).set("DisplayName", identity["display_name"])
    return tree


def prepare(binary, output, identity, package_version):
    imports = verify_binary(binary)
    tree = manifest(identity, package_version)
    output.mkdir(parents=True, exist_ok=False)  # Never clean or overwrite a previous candidate.
    layout = output / "layout"
    (layout / "Assets").mkdir(parents=True)
    tree.write(layout / "AppxManifest.xml", encoding="utf-8", xml_declaration=True)
    shutil.copy2(binary, layout / "FeatherMD.exe")
    for relative in FILES:
        dest = layout / relative
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, dest)
    from PIL import Image
    # Build resources from the existing application icon, preserving its design.
    with Image.open(ROOT / "assets/app.ico") as icon:
        source = icon.convert("RGBA")
        for name, size in SIZES.items():
            source.resize((size, size), Image.Resampling.LANCZOS).save(layout / "Assets" / f"{name}.png")
        for size in (16, 24, 32, 48, 256):
            source.resize((size, size), Image.Resampling.LANCZOS).save(
                layout / "Assets" / f"Square44x44Logo.targetsize-{size}_altform-unplated.png")
    return layout, imports


def verify_archive(package, layout):
    expected = {p.relative_to(layout).as_posix(): p for p in layout.rglob("*") if p.is_file()}
    with zipfile.ZipFile(package) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise ValueError("Duplicate MSIX entries")
        extras = set(names) - set(expected) - {"[Content_Types].xml", "AppxBlockMap.xml"}
        if extras or set(expected) - set(names):
            raise ValueError(f"Unexpected/missing MSIX content: {extras}, {set(expected) - set(names)}")
        for name, path in expected.items():
            if archive.read(name) != path.read_bytes():
                raise ValueError(f"MSIX content mismatch: {name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--version", required=True)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--test-identity", action="store_true")
    group.add_argument("--identity", type=Path)
    parser.add_argument("--makeappx", default="makeappx.exe")
    args = parser.parse_args()
    identity = TEST_IDENTITY if args.test_identity else json.loads(args.identity.read_text(encoding="utf-8-sig"))
    validate_identity(identity, production=not args.test_identity)
    output = args.output.resolve()
    layout, imports = prepare(args.binary.resolve(), output, identity, args.version)
    package = output / f"FeatherMD-{args.version}-x64{'-TEST' if args.test_identity else ''}.msix"
    subprocess.run([args.makeappx, "pack", "/d", str(layout), "/p", str(package), "/o"], check=True)
    verify_archive(package, layout)
    digest = hashlib.sha256(package.read_bytes()).hexdigest()
    (output / "SHA256SUMS.txt").write_text(f"{digest}  {package.name}\n", encoding="ascii")
    (output / "package-report.json").write_text(json.dumps({
        "identity": identity, "version": args.version, "test_only": args.test_identity,
        "signed": False, "installed_tested": False, "store_submission": False,
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "package_sha256": digest, "imports": imports,
    }, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"Verified unsigned package: {package}")


if __name__ == "__main__":
    main()
