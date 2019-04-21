#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import tempfile
from util import run, root_path

target_path = os.path.join(root_path, "target/")

os.chdir(root_path)

# Builds into target/doc
run(["cargo", "doc", "--all", "--no-deps", "-vv"])

# 'deno types' is stored in target/debug/gen/cli/lib/lib.deno_runtime.d.ts
# We want to run typedoc on that declaration file only.
os.chdir(os.path.join(target_path, "debug/gen/cli/lib/"))

# You must have typedoc installed seprately.
# TODO Replace typedoc with something else ASAP. It's very awful.
run([
    "typedoc", "lib.deno_runtime.d.ts", "--out",
    os.path.join(target_path, "typedoc"), "--entryPoint", "Deno",
    "--ignoreCompilerErrors", "--includeDeclarations", "--excludeExternals",
    "--excludePrivate", "--excludeProtected", "--mode", "file", "--name",
    "deno", "--theme", "minimal", "--readme", "none"
])
