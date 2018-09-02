#!/usr/bin/env python
# Run this script if you are changing Deno's dependencies.
# To update the deno_third_party git repo after running this, try the following:
# cd third_party
# find . -type f | grep -v "\.git" | xargs -I% git add -f --no-warn-embedded-repo "%"

import third_party
import util

util.enable_ansi_colors()

third_party.fix_symlinks()

third_party.run_yarn()
third_party.run_cargo()
third_party.run_gclient_sync()
