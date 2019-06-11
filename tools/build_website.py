#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
from util import run, root_path, build_path

os.chdir(os.path.join(root_path, "website"))
deno_exe = os.path.join(build_path(), "deno")
run([deno_exe, "bundle", "app.ts", "app.bundle.js"])
