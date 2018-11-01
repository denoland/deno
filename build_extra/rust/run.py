#!/usr/bin/env python
# This file just executes its arguments, except that also adds GN_OUT_DIR to the
# environ. This is for compatibility with cargo.
import subprocess
import sys
import os

# This is for src/msg.rs to know where to find msg_generated.rs.
# When building with Cargo this variable is set by build.rs.
os.environ["GN_OUT_DIR"] = os.path.abspath(".")
assert os.path.isdir(os.environ["GN_OUT_DIR"])

sys.exit(subprocess.call(sys.argv[1:]))
