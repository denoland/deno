#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
import sys
from util import run, root_path

os.chdir(root_path)
run([sys.executable, "tools/docs.py"])
os.chdir("target")
run([
    "aws", "s3", "sync", "--include=typedoc", "--exclude=debug/*",
    "--exclude=package/*", "--exclude=release/*", ".", "s3://deno.land/"
])
