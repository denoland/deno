#!/usr/bin/env bash
# Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

# It's important to tell cargo that the target-feature
# only applies to the target and not the host.
# https://github.com/rust-lang/rust/issues/78210
# Without explicit --target, proc_macro crates won't build.
#
# We also need to only build the binary crate (with `-p deno`)
# with the target-feature since our workspace consists of `cdylib`s.

RUSTFLAGS="-Ctarget-feature=+crt-static" cargo build \
  --target x86_64-unknown-linux-gnu \
  -p deno $@