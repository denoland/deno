#!/usr/bin/env python
"""
gn can only run python scripts.
protobuf.js must generate some javascript files.
it's very difficult to get this into the gn build sanely.
therefore we write them into the source directory.
"""
import subprocess
import sys
import os

js_path = os.path.dirname(os.path.realpath(__file__))
#bin_path = os.path.join(js_path, "deno_protobufjs", "bin")
bin_path = os.path.join(js_path, "node_modules", ".bin")
pbjs_bin = os.path.join(bin_path, "pbjs")
pbts_bin = os.path.join(bin_path, "pbts")
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
  pbjs_bin,
  #"--dependency=./deno_protobufjs/minimal",
  "--target=static-module",
  "--wraper=commonjs",
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
