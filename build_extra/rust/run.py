#!/usr/bin/env python
# This file just executes its arguments, except that it extracts a special
# argument --env-out-dir so as to pass it to the given command via environmental
# variable. This backflip is to have compatibility with cargo.
import subprocess
import argparse
import sys
import os

parser = argparse.ArgumentParser()
parser.add_argument("--env-out-dir", dest="env_out_dir")
flags, args_rest = parser.parse_known_args()

assert os.path.isdir(flags.env_out_dir)
os.environ["OUT_DIR"] = flags.env_out_dir

sys.exit(subprocess.call(args_rest, env=os.environ))
