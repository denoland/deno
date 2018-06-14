#!/usr/bin/env python
"""
gn can only run python scripts.

Generates protobufjs code.
"""
import subprocess
import sys
import os
# TODO(ry) Ideally protobufjs output files should be written into
# target_gen_dir, but its difficult to get this working in a way that parcel can
# resolve their location. (Parcel does not support NODE_PATH.) Therefore this
# hack: write the generated msg.pb.js and msg.pb.d.ts outputs into the js/
# folder, and we check them into the repo. Hopefully this hack can be removed at
# some point. If msg.proto is changed, commit changes to the generated JS
# files.

js_path = os.path.dirname(os.path.realpath(__file__))
pbjs_path = os.path.join(js_path, "node_modules", "protobufjs", "bin")
pbjs_bin = os.path.join(pbjs_path, "pbjs")
pbts_bin = os.path.join(pbjs_path, "pbts")
msg_pbjs_out = os.path.join(js_path, "msg.pb.js")
msg_pbts_out = os.path.join(js_path, "msg.pb.d.ts")
assert os.path.exists(pbjs_bin)
assert os.path.exists(pbts_bin)

proto_in = sys.argv[1]
stamp_file = sys.argv[2]

def touch(fname):
  if os.path.exists(fname):
    os.utime(fname, None)
  else:
    open(fname, 'a').close()

subprocess.check_call([
  "node",
  pbjs_bin,
  "--target=static-module",
  "--wrapper=commonjs",
  "--out=" + msg_pbjs_out,
  proto_in
])
assert os.path.exists(msg_pbjs_out)

subprocess.check_call([
  "node",
  pbts_bin,
  "--out=" + msg_pbts_out,
  msg_pbjs_out
])
assert os.path.exists(msg_pbts_out)

touch(stamp_file)
