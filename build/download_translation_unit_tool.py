#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Script to download Clang translation_unit tool from google storage."""

import find_depot_tools
import json
import os
import shutil
import subprocess
import sys
import tarfile

SCRIPT_DIR = os.path.dirname(os.path.realpath(__file__))
CHROME_SRC = os.path.abspath(os.path.join(SCRIPT_DIR, os.pardir))


DEPOT_PATH = find_depot_tools.add_depot_tools_to_path()
GSUTIL_PATH = os.path.join(DEPOT_PATH, 'gsutil.py')

LLVM_BUILD_PATH = os.path.join(CHROME_SRC, 'third_party', 'llvm-build',
                               'Release+Asserts')
CLANG_UPDATE_PY = os.path.join(CHROME_SRC, 'tools', 'clang', 'scripts',
                               'update.py')

CLANG_BUCKET = 'gs://chromium-browser-clang'


def main():
  clang_revision = subprocess.check_output([sys.executable, CLANG_UPDATE_PY,
                                            '--print-revision']).rstrip()
  targz_name = 'translation_unit-%s.tgz' % clang_revision

  if sys.platform == 'win32' or sys.platform == 'cygwin':
    cds_full_url = CLANG_BUCKET + '/Win/' + targz_name
  elif sys.platform == 'darwin':
    cds_full_url = CLANG_BUCKET + '/Mac/' + targz_name
  else:
    assert sys.platform.startswith('linux')
    cds_full_url = CLANG_BUCKET + '/Linux_x64/' + targz_name

  os.chdir(LLVM_BUILD_PATH)

  subprocess.check_call([sys.executable, GSUTIL_PATH,
                         'cp', cds_full_url, targz_name])
  tarfile.open(name=targz_name, mode='r:gz').extractall(path=LLVM_BUILD_PATH)

  os.remove(targz_name)
  return 0

if __name__ == '__main__':
  sys.exit(main())
