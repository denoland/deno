#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import tempfile
from util import run, root_path

# Probably run tools/docs.py first.
# AWS CLI must be installed separately.

os.chdir(os.path.join(root_path, "website"))

# Invalidate the cache.
run([
    "aws", "cloudfront", "create-invalidation", "--distribution-id",
    "E3NZSZMS5TZ0OU", "--paths", "/*"
])

run(["aws", "s3", "sync", ".", "s3://deno.land/"])
