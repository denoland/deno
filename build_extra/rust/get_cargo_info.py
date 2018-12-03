#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.

import os
import re

# Read the package version from Cargo.toml and output as json
current_path = os.path.dirname(os.path.realpath(__file__))
for line in open(os.path.join(current_path, "../../Cargo.toml")):
    match = re.search('version = "(.*)"', line)
    if match:
        print('{"version": "' + match.group(1) + '"}')
        break
