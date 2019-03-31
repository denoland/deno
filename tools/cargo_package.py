#!/usr/bin/env python
# Because of limitations in Cargo, Deno must dynamically build temporary source
# directories in order to publish to crates.io.
# The "deno" crate corresponds to the //core/ directory and depends on a
# platform dependent crate binary crate containing pre-compiled libdeno
# https://crates.io/crates/deno
# https://crates.io/crates/deno-x86_64-pc-windows-msvc
# https://crates.io/crates/deno-x86_64-apple-darwin
# https://crates.io/crates/deno-x86_64-unknown-linux-gnu

import os
import sys
import re
import errno
from shutil import copytree, ignore_patterns, copyfile
from tempfile import mkdtemp
from string import Template
from util import root_path
from util import run

if sys.platform == "linux2":
    llvm_target = "x86_64-unknown-linux-gnu"
    static_lib_suffix = ".a"
elif sys.platform == "darwin":
    llvm_target = "x86_64-apple-darwin"
    static_lib_suffix = ".a"
elif sys.platform == "win32":
    llvm_target = "x86_64-pc-windows-msvc"
    static_lib_suffix = ".lib"
else:
    assert (False)

lib_name = os.path.join(root_path, "target/release/obj/core/libdeno",
                        "libdeno" + static_lib_suffix)


def get_version(toml_path):
    for line in open(toml_path):
        match = re.search('version = "(.*)"', line)
        if match:
            return match.group(1)


core_path = os.path.join(root_path, "core")
cargo_toml_path = os.path.join(core_path, "Cargo.toml")
version = get_version(cargo_toml_path)


def main():
    os.chdir(root_path)

    run(["tools/build.py", "libdeno_static_lib", "--release"])
    assert (os.path.exists(lib_name))

    root_temp = mkdtemp()
    print "cargo package temp dir", root_temp

    build_core(root_temp)
    build_platform_crate(root_temp)

    print "Now go into %s and run 'cargo publish' manually." % root_temp


def build_core(root_temp):
    core_temp = os.path.join(root_temp, "core")

    # Copy entire core tree into temp directory, excluding build.rs and libdeno
    # and unnecessary files.
    copytree(
        core_path,
        core_temp,
        ignore=ignore_patterns("build.rs", "libdeno", ".*", "*.gn", "*.orig"))

    cargo_toml_temp = os.path.join(core_temp, "Cargo.toml")

    t = cargo_toml_deps.substitute(VERSION=version)
    # Append deps to //core/Cargo.toml
    # This append is the entire reason we are copying the tree.
    with open(cargo_toml_temp, "a") as f:
        f.write(t)


def build_platform_crate(root_temp):
    platform_temp = os.path.join(root_temp, "platform")

    copy_static_lib(platform_temp)

    inputs = {"TARGET": llvm_target, "VERSION": version}

    generate(platform_temp, "build.rs", platform_build_rs.substitute(inputs))
    generate(platform_temp, "Cargo.toml",
             platform_cargo_toml.substitute(inputs))
    generate(platform_temp, "src/lib.rs", "")


def copy_static_lib(platform_temp):
    platform_lib = os.path.join(platform_temp, "lib/")
    mkdir_p(platform_lib)
    platform_lib_name = os.path.join(platform_lib, os.path.basename(lib_name))
    assert (os.path.exists(lib_name))
    copyfile(lib_name, platform_lib_name)


platform_build_rs = Template("""
fn main() {
  use std::env::var;
  use std::path::Path;
  if var("TARGET")
    .map(|target| target == "$TARGET")
    .unwrap_or(false)
  {
    let dir = var("CARGO_MANIFEST_DIR").unwrap();
    println!(
      "cargo:rustc-link-search=native={}",
      Path::new(&dir).join("lib").display()
    );
  }
}
""")

platform_cargo_toml = Template("""
[package]
name = "deno-$TARGET"
description = "Binary dependencies for the 'deno' crate"
authors = ["The deno authors <bertbelder@nodejs.org>"]
version = "$VERSION"
build = "build.rs"
include = ["src/*", "lib/*", "Cargo.toml", "build.rs"]
license = "MIT"
repository = "https://github.com/denoland/deno"
""")

cargo_toml_deps = Template("""
[target.x86_64-apple-darwin.dependencies]
deno-x86_64-apple-darwin = "=$VERSION"
  
[target.x86_64-pc-windows-msvc.dependencies]
deno-x86_64-pc-windows-msvc = "=$VERSION"

[target.x86_64-unknown-linux-gnu.dependencies]
deno-x86_64-unknown-linux-gnu = "=$VERSION"
""")


def mkdir_p(path):
    try:
        os.makedirs(path)
    except OSError as exc:
        if exc.errno == errno.EEXIST and os.path.isdir(path):
            pass
        else:
            raise


def generate(out_dir, filename, content):
    path = os.path.join(out_dir, filename)
    d = os.path.dirname(path)
    mkdir_p(d)
    with open(path, "w") as f:
        f.write(content)


if __name__ == '__main__':
    sys.exit(main())
