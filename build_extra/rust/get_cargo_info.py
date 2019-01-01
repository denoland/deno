#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.

import sys
import re

# Read the package version from Cargo.toml and output as json
cargo_toml_path = sys.argv[1]

for line in open(cargo_toml_path):
    match = re.search('version = "(.*)"', line)
    if match:
        print('{"version": "' + match.group(1) + '"}')
        break
