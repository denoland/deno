#!/usr/bin/env python
# This file just executes its arguments, except that also adds OUT_DIR to the
# environ. This is for compatibility with cargo.
import subprocess
import sys
import os

os.environ["OUT_DIR"] = os.path.abspath(".")
assert os.path.isdir(os.environ["OUT_DIR"])
sys.exit(subprocess.call(sys.argv[1:], env=os.environ))
