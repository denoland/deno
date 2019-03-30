import sys
import argparse
import re
import os
import errno
from string import Template

parser = argparse.ArgumentParser()
parser.add_argument("--root")
parser.add_argument("--target")
parser.add_argument("--out")

args = parser.parse_args()

print args


def get_version(core_cargo_toml_path):
    for line in open(core_cargo_toml_path + "/core/Cargo.toml"):
        match = re.search('version = "(.*)"', line)
        if match:
            return match.group(1)


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
    dir = os.path.dirname(path)
    mkdir_p(dir)
    with open(path, "w") as f:
        f.write(content)


inputs = {"TARGET": args.target, "VERSION": get_version(args.root)}

build_rs = Template("""
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
""").substitute(inputs)

cargo_toml = Template("""
[package]
name = "deno-$TARGET"
description = "Binary dependencies for the 'deno' crate"
authors = ["The deno authors <bertbelder@nodejs.org>"]
version = "$VERSION"
build = "build.rs"
include = ["src/*", "lib/*", "Cargo.toml", "build.rs"]
license = "MIT"
repository = "https://github.com/denoland/deno"
""").substitute(inputs)

generate(args.out, "build.rs", build_rs)
generate(args.out, "Cargo.toml", cargo_toml)
generate(args.out, "src/lib.rs", "")
