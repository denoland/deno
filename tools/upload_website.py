#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import sys
import tempfile
from util import run, root_path, build_path

# Probably run tools/docs.py first.
# AWS CLI must be installed separately.

os.chdir(os.path.join(root_path, "website"))

deno_exe = os.path.join(build_path(), "deno")
run([sys.executable, "../tools/build_website.py"])

# Invalidate the cache.
run([
    "aws", "cloudfront", "create-invalidation", "--distribution-id",
    "E2HNK8Z3X3JDVG", "--paths", "/*"
])

run(["aws", "s3", "sync", ".", "s3://deno.land/"])
