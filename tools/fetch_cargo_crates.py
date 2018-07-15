#!/usr/bin/env python
# Inspired by
# https://fuchsia.googlesource.com/build/+/master/rust/list_3p_crates.py
# https://fuchsia.googlesource.com/build/+/master/rust/compile_3p_crates.py
# Copyright 2018 The Fuchsia Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
from os.path import join, dirname, realpath
import sys
import subprocess

root_path = dirname(dirname(realpath(__file__)))
third_party_path = join(root_path, "third_party")

sys.path += [join(third_party_path, "pytoml")]
import pytoml

def run_command(args, env, cwd):
    job = subprocess.Popen(args, env=env, cwd=cwd, stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE)
    stdout, stderr = job.communicate()
    return (job.returncode, stdout, stderr)

def clone_crate(name, version):
    call_args = [
        "cargo", "clone",
        "%s:%s" % (name, version)
    ]
    env = os.environ.copy()
    _, _, stderr = run_command(
        call_args,
        env,
        join(third_party_path, "rust_crates")
    )
    print(stderr)

def main():
    cargo_toml_path = join(third_party_path, "Cargo.toml")
    with open(cargo_toml_path, "r") as file:
        cargo_toml = pytoml.load(file)
        for key, value in cargo_toml["dependencies"].items():
            if type(value) is dict:
                git = value.get("git")
                clone_crate(key, git)
            else:
                clone_crate(key, value)

if __name__ == '__main__':
    sys.exit(main())
