#!/usr/bin/env python
"""
gn can only run python scripts.

Generates flatbuffer TypeScript code.
"""
import subprocess
import sys
import os
import shutil
import util

# TODO(ry) Ideally flatc output files should be written into target_gen_dir, but
# its difficult to get this working in a way that parcel can resolve their
# location. (Parcel does not support NODE_PATH.) Therefore this hack: write the
# generated msg_generated.ts outputs into the js/ folder, and we check them into
# the repo. Hopefully this hack can be removed at some point. If msg.fps is
# changed, commit changes to the generated JS file.

src = sys.argv[1]
dst = sys.argv[2]
stamp_file = sys.argv[3]

shutil.copyfile(src, dst)

util.touch(stamp_file)
