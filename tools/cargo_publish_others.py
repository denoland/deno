#!/usr/bin/env python
# Publishes 'deno_cli', 'deno_cli_snapshots', and 'deno_typescript' crates.
# DOES NOT PUBLISH 'deno' crate see tools/cargo_package.py for that.

import os
import sys
import argparse
from util import run, root_path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    cargo_publish = ["cargo", "publish"]
    if args.dry_run:
        cargo_publish += ["--dry-run"]

    # Publish the deno_typescript crate.
    os.chdir(os.path.join(root_path, "deno_typescript"))
    run(cargo_publish)

    # Publish the deno_cli_snapshots crate.
    os.chdir(os.path.join(root_path, "js"))
    run(cargo_publish)

    # Publish the deno_cli crate.
    os.chdir(os.path.join(root_path, "cli"))
    run(cargo_publish)


if __name__ == '__main__':
    sys.exit(main())
