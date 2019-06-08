# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import sys
import third_party
from util import run_output, build_path

out_filename = sys.argv[1]

args_list = run_output([
    third_party.gn_path, "args",
    build_path(), "--list", "--short", "--overrides-only"
],
                       quiet=True,
                       env=third_party.google_env(),
                       exit_on_fail=True).out

with open(out_filename, "w") as f:
    f.write(args_list)
