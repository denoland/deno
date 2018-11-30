# Copyright 2018 the Deno authors. All rights reserved. MIT license.
"""
Computes the SHA256 hash and formats the result.
"""

import argparse
from hashlib import sha256
import os
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)

    # Arguments specifying where input comes from.
    # If multiple sources are specified, they are all concatenated together.
    parser.add_argument(
        "--input",
        action="append",
        dest="input",
        type=str,
        metavar="TEXT",
        help="Hash literal text specified on the command line.")
    parser.add_argument(
        "--infile",
        action="append",
        dest="input",
        type=read_file,
        metavar="FILE",
        help="Hash the contents of a file.")

    # Arguments dealing with output.
    parser.add_argument(
        "--format",
        type=str,
        dest="format",
        default="%s",
        metavar="TEMPLATE",
        help="Format output using Python template (default = '%%s').")
    parser.add_argument(
        "--outfile",
        dest="outfile",
        type=argparse.FileType("wb"),
        default=sys.stdout,
        metavar="FILE",
        help="Write the formatted hash to a file (default = stdout).")

    # Parse arguments. Print usage and exit if given no input.
    args = parser.parse_args()
    if (not args.input):
        parser.print_usage()
        return 1

    # Compute the hash of all inputs concatenated together.
    hasher = sha256()
    for data in args.input:
        hasher.update(data)
    h = hasher.hexdigest()

    # Format and write to specified out file (or the default, stdout).
    args.outfile.write(args.format % h)


def read_file(filename):
    with open(filename, "rb") as f:
        return f.read()


if __name__ == '__main__':
    sys.exit(main())
