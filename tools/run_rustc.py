#!/usr/bin/env python
# Inspired by
# https://fuchsia.googlesource.com/build/+/master/rust/build_rustc_target.py
# Copyright 2018 The Fuchsia Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
import sys
import os
import argparse
import subprocess
import util


# Updates the path of the main target in the depfile to the relative path
# from base_path build_output_path
def fix_depfile(depfile_path, base_path, build_output_path):
    with open(depfile_path, "r") as depfile:
        content = depfile.read()
    content_split = content.split(': ', 1)
    target_path = os.path.relpath(build_output_path, start=base_path)
    new_content = "%s: %s" % (target_path, content_split[1])
    with open(depfile_path, "w") as depfile:
        depfile.write(new_content)


def main():
    parser = argparse.ArgumentParser("Compiles a Rust crate")
    parser.add_argument(
        "--depfile",
        help="Path at which the output depfile should be stored",
        required=False)
    parser.add_argument(
        "--output_file",
        help="Path at which the output file should be stored",
        required=False)
    args, rest = parser.parse_known_args()

    util.run(["rustc"] + rest, quiet=True)

    if args.depfile and args.output_file:
        fix_depfile(args.depfile, os.getcwd(), args.output_file)


if __name__ == '__main__':
    sys.exit(main())
