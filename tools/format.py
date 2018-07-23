#!/usr/bin/env python
import os
from util import run, find_exts

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = os.path.join(root_path, "third_party")
prettier = os.path.join(third_party_path, "node_modules", "prettier",
                        "bin-prettier.js")
tools_path = os.path.join(root_path, "tools")
rustfmt_config = os.path.join(tools_path, "rustfmt.toml")

os.chdir(root_path)

# TODO(ry) Use third_party/depot_tools/clang-format.
run(["clang-format", "-i", "-style", "Google"] + find_exts("src", ".cc", ".h"))

for fn in ["BUILD.gn", ".gn"] + find_exts("build_extra", ".gn", ".gni"):
    run(["third_party/depot_tools/gn", "format", fn])

# TODO(ry) Install yapf in third_party.
run(["yapf", "-i"] + find_exts("tools/", ".py") +
    find_exts("build_extra", ".py"))

run(["node", prettier, "--write"] + find_exts("js/", ".js", ".ts") +
    ["tsconfig.json", "tslint.json"])

# Set RUSTFMT_FLAGS for extra flags.
rustfmt_extra_args = []
if 'RUSTFMT_FLAGS' in os.environ:
    rustfmt_extra_args += os.environ['RUSTFMT_FLAGS'].split()
run([
    "rustfmt", "--config-path", rustfmt_config, "--error-on-unformatted",
    "--write-mode", "overwrite"
] + rustfmt_extra_args + find_exts("src/", ".rs"))
