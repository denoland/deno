# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Copies a FVM file and extends it by a specified amount.

Arg #1: path to 'fvm'.
    #2: the path to the source fvm.blk.
    #3: the path that the extended FVM file will be written to.
    #4: the additional number of bytes to grow fvm.blk by."""

import os
import shutil
import subprocess
import sys

def ExtendFVM(fvm_tool_path, src_path, dest_path, delta):
  old_size = os.path.getsize(src_path)
  new_size = old_size + int(delta)
  shutil.copyfile(src_path, dest_path)
  subprocess.check_call([fvm_tool_path, dest_path, 'extend', '--length',
                         str(new_size)])
  return 0

if __name__ == '__main__':
  sys.exit(ExtendFVM(*sys.argv[1:]))
