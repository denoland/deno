#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import argparse
import os
import sys
from os.path import join
from util import run
import distutils.spawn

parser = argparse.ArgumentParser(description='')
parser.add_argument(
    '--build_path', default='', help='Directory to build into.')
parser.add_argument(
    '--args', default='', help='Specifies build arguments overrides.')
parser.add_argument(
    '--mode', default='debug', help='Build configuration: debug, release.')
options, targets = parser.parse_known_args()

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = join(root_path, "third_party")
depot_tools_path = join(third_party_path, "depot_tools")
gn_path = join(depot_tools_path, "gn")
ninja_path = join(depot_tools_path, "ninja")

# Add third_party/depot_tools to PATH  because some google tools (e.g.
# tool_wrapper, download_from_google_storage) use some google specific python
# wrapper.
os.environ["PATH"] = depot_tools_path + os.pathsep + os.environ["PATH"]

os.chdir(root_path)

if options.build_path:
    build_path = options.build_path
else:
    build_path = join(root_path, "out", options.mode)

gn_args = []
if options.args:
    gn_args += options.args.split()

if options.mode == "release":
    gn_args += ["is_official_build=true"]
elif options.mode == "debug":
    pass
else:
    print "Bad mode {}. Use 'release' or 'debug' (default)" % options.mode
    sys.exit(1)

# Check if ccache is in the path, and if so we cc_wrapper.
ccache_path = distutils.spawn.find_executable("ccache")
if ccache_path:
    gn_args += [r'cc_wrapper="%s"' % ccache_path]

# mkdir $build_path. We do this so we can write args.gn before running gn gen.
if not os.path.isdir(build_path):
    os.makedirs(build_path)

# Rather than using gn gen --args we manually write the args.gn override file.
# This is to avoid quoting/escaping complications when passing overrides as
# command-line arguments.
args_filename = join(build_path, "args.gn")
if not os.path.exists(args_filename) or options.args:
    with open(args_filename, "w+") as f:
        f.write("\n".join(gn_args) + "\n")

run([gn_path, "gen", build_path])

target = " ".join(targets) if targets else ":all"
run([ninja_path, "-C", build_path, target])
