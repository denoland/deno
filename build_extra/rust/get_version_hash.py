# Copyright 2018 the Deno authors. All rights reserved. MIT license.

# This script computes the sha256sum of the first command line argument, and
# writes a few hex digits of it to stdout. It is used by rust.gni to derive a
# unique string (without dots/special characters) from a crate version number.

from hashlib import sha256
import sys

if len(sys.argv) != 2:
    raise Exception('Expected exactly one argument.')

hash = sha256(sys.argv[1]).hexdigest()
sys.stdout.write(hash[0:8])
