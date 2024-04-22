#!/usr/bin/env python3

import argparse
import os
import re
import subprocess
from typing import Iterable

RE_VERSION = re.compile(r"^([^ #=]+)==([^ #]+)$")

FILES = [
    "tests/openapi/requirements-freeze.txt",
    "tests/consensus_tests/requirements.txt",
]

# https://github.com/qdrant/sparse-vectors-benchmark/blob/master/requirements.txt
SPARSE_VECTOR_BENCHMARK_REQS = """
qdrant_client==1.8.2
scipy==1.12.0
numpy==1.26.4
tqdm==4.66.2
click==8.1.7
matplotlib==3.8.3
requests==2.31.0
"""

PREAMBLE = """
# This file is autogenerated by ./regen.py

[tool.poetry]
name = "qdrant-dev"
version = "0.1.0"
description = ""
authors = ["Your Name <you@example.com>"]

[build-system]
requires = ["poetry-core"]
build-backend = "poetry.core.masonry.api"

[tool.poetry.dependencies]
python = "^3.11"
""".lstrip()

OVERRIDES = {
    "qdrant_client": "1.8.0",  # 1.3.1 is too old
    # See https://github.com/nix-community/poetry2nix/blob/master/overrides/default.nix
    # for supported versions of cryptography
    "cryptography": "42.0.3",  # 42.0.5
    "schemathesis": "3.25.6",  # 3.24.3 conflicts with pytest>=8
    "hypothesis-jsonschema": "0.23.1",  # 0.23.0, schemathesis wants >=0.23.1
}


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Regenerate pyproject.toml and poetry.lock."
    )
    parser.add_argument("repo", help="Location of the cloned qdrant repo")
    args = parser.parse_args()

    deps = dict[str, str]()

    for file in FILES:
        with open(f"{args.repo}/{file}") as f:
            add_deps(deps, f)
    add_deps(deps, SPARSE_VECTOR_BENCHMARK_REQS.splitlines())

    for k, v in OVERRIDES.items():
        if k in deps:
            deps[k] = v

    script_dir = os.path.dirname(os.path.realpath(__file__))

    with open(f"{script_dir}/pyproject.toml", "w") as f:
        f.write(PREAMBLE)
        for m in sorted(deps.items()):
            f.write(f"{m[0]} = {m[1]!r}\n")

    subprocess.run(
        # ["nix-shell", "-p", "poetry", "--run", "poetry lock --no-update"],
        ["poetry", "lock", "--no-update"],
        cwd=script_dir,
        check=True,
    )


def add_deps(deps: dict[str, str], lines: Iterable[str]) -> None:
    for line in lines:
        if m := RE_VERSION.match(line.rstrip("\r\n")):
            deps[m[1]] = m[2]


if __name__ == "__main__":
    main()
