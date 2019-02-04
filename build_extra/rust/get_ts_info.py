#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import sys
import re

# Read the package version from package.json and output as json
package_json_path = sys.argv[1]

for line in open(package_json_path):
    match = re.search('"typescript": "(.*)"', line)
    if match:
        print('{"version": "' + match.group(1) + '"}')
        break
