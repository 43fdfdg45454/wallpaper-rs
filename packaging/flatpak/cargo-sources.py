#!/usr/bin/env python3
import json
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"
CONFIG = '[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = "cargo/vendor"\n'

sources = []
for p in tomllib.loads((HERE.parent.parent / "Cargo.lock").read_text())["package"]:
    if "source" not in p:
        continue
    if p["source"] != CRATES_IO:
        sys.exit(f"{p['name']} {p['version']}: only crates.io packages can be vendored")
    name, version, sha = p["name"], p["version"], p["checksum"]
    dest = f"cargo/vendor/{name}-{version}"
    sources.append({
        "type": "archive",
        "archive-type": "tar-gzip",
        "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
        "sha256": sha,
        "dest": dest,
    })
    sources.append({
        "type": "inline",
        "contents": json.dumps({"package": sha, "files": {}}),
        "dest": dest,
        "dest-filename": ".cargo-checksum.json",
    })
sources.append({"type": "inline", "contents": CONFIG, "dest": "cargo", "dest-filename": "config.toml"})
(HERE / "cargo-sources.json").write_text(json.dumps(sources, indent=2) + "\n")
