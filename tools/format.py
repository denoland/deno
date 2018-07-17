#!/usr/bin/env python

import os
from glob import glob
from util import run

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = os.path.join(root_path, "third_party")
prettier = os.path.join(third_party_path, "node_modules", "prettier",
                        "bin-prettier.js")

os.chdir(root_path)
# TODO(ry) Install clang-format in third_party.
run(["clang-format", "-i", "-style", "Google"] + glob("src/*.cc") +
    glob("src/*.h"))
for fn in ["BUILD.gn", ".gn"] + glob("build_extra/**/*.gn*"):
    run(["gn", "format", fn])
# TODO(ry) Install yapf in third_party.
run(["yapf", "-i"] + glob("tools/*.py") + glob("build_extra/**/*.py"))
run(["node", prettier, "--write"] + glob("js/*.js") + glob("js/*.ts") +
    ["tsconfig.json"] + ["tslint.json"])

# Set RUSTFMT_FLAGS for extra flags.
rustfmt_extra_args = []
if 'RUSTFMT_FLAGS' in os.environ:
    rustfmt_extra_args += os.environ['RUSTFMT_FLAGS'].split()
run(["rustfmt", "--write-mode", "overwrite"] + rustfmt_extra_args +
    glob("src/*.rs"))
