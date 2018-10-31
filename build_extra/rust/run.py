#!/usr/bin/env python
# This file just executes its arguments, except that also adds OUT_DIR to the
# environ. This is for compatibility with cargo.
import subprocess
import sys
import os

# TODO This is for src/msg.rs to know where to find msg_generated.rs
# In the future we should use OUT_DIR here.
os.environ["DENO_BUILD_PATH"] = os.path.abspath(".")
assert os.path.isdir(os.environ["DENO_BUILD_PATH"])

sys.exit(subprocess.call(sys.argv[1:], env=os.environ))
