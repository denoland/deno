#!/usr/bin/env python

import os
from glob import glob
from util import run

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))

os.chdir(root_path)
# TODO(ry) Install clang-format in third_party.
run(["clang-format", "-i", "-style", "Google"] + glob("src/*.cc") +
    glob("src/*.h"))
for fn in ["BUILD.gn", ".gn"] + glob("build_extra/**/*.gn*"):
    run(["gn", "format", fn])
# TODO(ry) Install yapf in third_party.
run(["yapf", "-i"] + glob("tools/*.py"))
# TODO(ry) Install prettier in third_party.
run([
    "prettier", "--write", "js/deno.d.ts", "js/main.ts", "js/mock_runtime.js",
    "tsconfig.json"
])
# Do not format these.
#  js/msg_generated.ts
#  js/flatbuffers.js
run(["rustfmt", "-f", "--write-mode", "overwrite"] + glob("src/*.rs"))
