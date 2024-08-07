#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib
import typing
import urllib.request


def main() -> None:
    parser = argparse.ArgumentParser(description="Update the versions.json file")
    parser.add_argument("--nightly", type=str, help="nightly version, e.g. 2021-01-01")
    parser.add_argument("--stable", type=str, help="stable version, e.g. 1.76.0")
    parser.add_argument(
        "--qdrant", type=str, help="qdrant revision, e.g. v1.10.1 or master"
    )
    args = parser.parse_args()

    d = f"{args.nightly}/" if args.nightly else ""
    url = f"https://static.rust-lang.org/dist/{d}channel-rust-nightly.toml"
    with urllib.request.urlopen(url) as response:
        data = response.read()
    nightly_sha256 = hashlib.sha256(data).digest().hex()
    nightly_date = tomllib.loads(data.decode("utf-8"))["date"]

    d = args.stable if args.stable else "stable"
    url = f"https://static.rust-lang.org/dist/channel-rust-{d}.toml"
    with urllib.request.urlopen(url) as response:
        data = response.read()
    stable_sha256 = hashlib.sha256(data).digest().hex()
    stable_version = tomllib.loads(data.decode("utf-8"))["pkg"]["rust"]["version"]
    m = re.match(r"^(\d+\.\d+\.\d+) \(.*\)$", stable_version)
    if not m:
        raise ValueError(f"Failed to parse stable version: {stable_version}")
    stable_version = m[1]

    qdrant = json.loads(
        subprocess.check_output(
            [
                "nix-prefetch-github",
                "qdrant",
                "qdrant",
                "--rev",
                args.qdrant or "master",
            ]
        )
    )

    result = {
        "#": "This file is autogenerated by ./update.py",
        "nightly": {
            "date": nightly_date,
            "sha256": nightly_sha256,
        },
        "stable": {
            "version": stable_version,
            "sha256": stable_sha256,
        },
        "qdrant": qdrant,
    }

    with open("versions.json", "w") as f:
        f.write(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
