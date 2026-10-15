#!/usr/bin/env python3
"""Write a CycloneDX inventory of registry crates in the mobile web runtime."""
import json
import re
import subprocess
import tomllib
from pathlib import Path
from urllib.parse import quote

root = Path(__file__).resolve().parents[1] / "godot-explorer" / "lib"
lock = tomllib.loads((root / "Cargo.lock").read_text())
registry = {
    (package["name"], package["version"])
    for package in lock["package"]
    if package.get("source", "").startswith("registry+")
}
tree = subprocess.check_output([
    "cargo", "tree", "--locked", "--target", "wasm32-unknown-emscripten",
    "--no-default-features", "--features", "use_pulse", "--edges", "normal", "--prefix", "none", "--format", "{p}",
], cwd=root, text=True)
selected = {
    (match[1], match[2])
    for line in tree.splitlines()
    if (match := re.match(r"^([A-Za-z0-9_-]+) v([^ ]+)", line))
} & registry
if not selected:
    raise SystemExit("No browser dependencies found")
print(json.dumps({
    "bomFormat": "CycloneDX", "specVersion": "1.6", "version": 1,
    "components": [
        {"type": "library", "name": name, "version": version,
         "purl": f"pkg:cargo/{quote(name)}@{quote(version)}"}
        for name, version in sorted(selected)
    ],
}, indent=2))
