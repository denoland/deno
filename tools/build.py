#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import sys
from os.path import join
import third_party
from util import root_path, run, run_output, build_path

third_party.fix_symlinks()

print "DENO_BUILD_PATH:", build_path()
if not os.path.isdir(build_path()):
    print "DENO_BUILD_PATH does not exist. Run tools/setup.py"
    sys.exit(1)
os.chdir(build_path())


def maybe_add_default_target(args):
    lines = run_output(
        [third_party.ninja_path, "-t", "targets"],
        env=third_party.google_env(),
        quiet=True).split("\n")
    targets = [l.rsplit(":", 1)[0] for l in lines]
    deno_targets = [target for target in targets if target.startswith(":")]
    deno_targets += [target.lstrip(":") for target in deno_targets]

    target_specified = False
    for a in args:
        if a in deno_targets:
            target_specified = True
            break
    if not target_specified:
        args += [":all"]
    return args


ninja_args = maybe_add_default_target(sys.argv[1:])

run([third_party.ninja_path] + ninja_args,
    env=third_party.google_env(),
    quiet=True)
