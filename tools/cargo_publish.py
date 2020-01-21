#!/usr/bin/env python
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

    os.chdir(os.path.join(root_path, "core"))
    run(cargo_publish)

    os.chdir(os.path.join(root_path, "deno_typescript"))
    run(cargo_publish)

    os.chdir(os.path.join(root_path, "cli"))
    run(cargo_publish)


if __name__ == '__main__':
    sys.exit(main())
