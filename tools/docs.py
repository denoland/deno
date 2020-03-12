#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
import tempfile
from util import run, root_path

target_path = os.path.join(root_path, "target/")

# 'deno types' is stored in js/lib.deno_runtime.d.ts
# We want to run typedoc on that declaration file only.
os.chdir(os.path.join(root_path, "cli/js"))

# You must have typedoc installed seprately.
# TODO Replace typedoc with something else ASAP. It's very awful.
run([
    "typedoc", "lib.deno.ns.d.ts", "--out",
    os.path.join(target_path, "typedoc"), "--entryPoint", "Deno",
    "--ignoreCompilerErrors", "--includeDeclarations", "--excludeExternals",
    "--excludePrivate", "--excludeProtected", "--mode", "file", "--name",
    "deno", "--theme", "minimal", "--readme", "none"
])
