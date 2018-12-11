#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import tempfile
from util import run, root_path

target_path = os.path.join(root_path, "target/")
tsconfig_path = os.path.join(root_path, "tsconfig.docs.json")
print(tsconfig_path)
os.chdir(root_path)

# Builds into target/doc
run(["cargo", "doc", "--no-deps", "-vv"])

# 'deno --types' is stored in target/debug/gen/lib/lib.deno_runtime.d.ts
# We want to run typedoc on that declaration file only.
os.chdir(os.path.join(target_path, "debug/gen/lib/"))

# You must have compodoc installed seprately.
# TODO Replace compodoc with something else ASAP. It's very awful.
run([
    "compodoc", "-p", tsconfig_path, "--output",
    os.path.join(target_path, "compodoc")
], None, root_path)