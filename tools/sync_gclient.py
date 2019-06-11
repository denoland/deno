#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# Run this script if you are changing //gclient_config.py
# To update the deno_third_party git repo after running this, try the following:
# cd third_party
# find v8 -type f | grep -v "\.git" | \
#   xargs -I% git add -f --no-warn-embedded-repo "%"

import third_party
import util

util.enable_ansi_colors()
third_party.run_gclient_sync()
