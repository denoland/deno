import os
import re
from shutil import copytree, ignore_patterns
from tempfile import mkdtemp
from string import Template
from util import root_path


def get_version(cargo_toml_path):
    for line in open(cargo_toml_path):
        match = re.search('version = "(.*)"', line)
        if match:
            return match.group(1)


root_temp = mkdtemp()

core_path = os.path.join(root_path, "core")
core_temp = os.path.join(root_temp, "core")

copytree(
    core_path,
    core_temp,
    ignore=ignore_patterns("build.rs", "libdeno", ".*", "*.gn", "*.orig"))

cargo_toml_path = os.path.join(core_path, "Cargo.toml")
cargo_toml_temp = os.path.join(core_temp, "Cargo.toml")

version = get_version(cargo_toml_path)
cargo_toml_deps_text = Template("""
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

[package]
name = "deno"
version = "$VERSION"
edition = "2018"
description = "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio"
authors = ["The deno authors <bertbelder@nodejs.org>"]
license = "MIT"
repository = "https://github.com/denoland/deno"

[lib]
path = "lib.rs"

[dependencies]
futures = "0.1.25"
lazy_static = "1.3.0"
libc = "0.2.51"
log = "0.4.6"
serde_json = "1.0.39"

[target.x86_64-apple-darwin.dependencies]
deno-x86_64-pc-windows-msvc = "$VERSION"
  
[target.x86_64-pc-windows-msvc.dependencies]
deno-x86_64-pc-windows-msvc = "$VERSION"

[target.x86_64-unknown-linux-gnu.dependencies]
deno-x86_64-unknown-linux-gnu = "$VERSION"
""").substitute(VERSION=version)

with open(cargo_toml_temp, "w") as f:
    f.write(cargo_toml_deps_text)

print core_temp
