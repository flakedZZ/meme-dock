"""Create Tauri's signed update feed from both release builds."""

import argparse
import json
from pathlib import Path
from urllib.parse import quote


def make_manifest(assets: Path, tag: str, repository: str) -> dict:
    version = tag.removeprefix("v")
    platforms = {}
    for target, filename in {
        "darwin-aarch64": "MemeDock.app.tar.gz",
        "windows-x86_64-nsis": f"MemeDock_{version}_x64-setup.exe",
        "windows-x86_64-msi": f"MemeDock_{version}_x64_en-US.msi",
    }.items():
        bundles = list(assets.rglob(filename))
        if len(bundles) != 1:
            raise ValueError(f"Expected one {filename}, found {len(bundles)}")
        sig_file = bundles[0].with_name(f"{filename}.sig")
        signature = sig_file.read_text(encoding="utf-8").strip()
        if not signature:
            raise ValueError(f"Empty signature: {sig_file}")
        platforms[target] = {
            "signature": signature,
            "url": f"https://github.com/{repository}/releases/download/{quote(tag)}/{quote(filename)}",
        }
    platforms["windows-x86_64"] = platforms["windows-x86_64-nsis"]
    return {"version": version, "platforms": platforms}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("assets", type=Path)
    parser.add_argument("tag")
    parser.add_argument("repository")
    args = parser.parse_args()
    manifest = make_manifest(args.assets, args.tag, args.repository)
    (args.assets / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
