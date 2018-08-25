#!/usr/bin/env python
import os
from third_party import third_party_path, fix_symlinks, google_env, clang_format_path
from util import root_path, run, find_exts

fix_symlinks()

prettier = os.path.join(third_party_path, "node_modules", "prettier",
                        "bin-prettier.js")
tools_path = os.path.join(root_path, "tools")
rustfmt_config = os.path.join(tools_path, "rustfmt.toml")

os.chdir(root_path)

run([clang_format_path, "-i", "-style", "Google"] +
    find_exts("libdeno", ".cc", ".h"))

for fn in ["BUILD.gn", ".gn"] + find_exts("build_extra", ".gn", ".gni"):
    run(["third_party/depot_tools/gn", "format", fn], env=google_env())

# TODO(ry) Install yapf in third_party.
run(["yapf", "-i"] + find_exts("tools/", ".py") +
    find_exts("build_extra", ".py"))

run(["node", prettier, "--write"] + find_exts("js/", ".js", ".ts") +
    find_exts("tests/", ".js", ".ts") +
    ["rollup.config.js", "tsconfig.json", "tslint.json"])

# Requires rustfmt 0.8.2 (flags were different in previous versions)
run(["rustfmt", "--config-path", rustfmt_config] + find_exts("src/", ".rs"))
