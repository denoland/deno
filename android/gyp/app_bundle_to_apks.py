#!/usr/bin/env python
#
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Process an app bundle (.aab) file into a set of split APKS (.apks)"""

import argparse
import itertools
import os
import shutil
import sys
import tempfile
import zipfile

# NOTE: Keep this consistent with the _app_bundle_to_apk_py_imports definition
#       in build/config/android/rules.gni
from util import build_utils
import bundletool

def _ParseArgs(args):
  parser = argparse.ArgumentParser(description=__doc__)

  parser.add_argument('--aapt2', required=True,
                      help='Path to aapt2 tool')
  parser.add_argument('--bundle', required=True,
                      help='Input bundle file.')
  parser.add_argument('--out-zip', required=True,
                      help='Output zip archive that will contain all APKs.')
  parser.add_argument('--keystore-path', required=True,
                      help='Keystore path')
  parser.add_argument('--keystore-password', required=True,
                      help='Keystore password')
  parser.add_argument('--key-name', required=True,
                      help='Keystore key name')

  options = parser.parse_args(args)

  return options


def main(args):
  args = build_utils.ExpandFileArgs(args)
  options = _ParseArgs(args)

  with build_utils.TempDir() as tmp_dir:
    # NOTE: The bundletool build-apks command requires the --output
    #       path to not exist, and to end with '.apks'.
    tmp_bundle = os.path.join(tmp_dir,
                              os.path.basename(options.bundle) + '.apks')

    cmd_args = ['java', '-jar', bundletool.BUNDLETOOL_JAR_PATH, 'build-apks']
    cmd_args += ['--aapt2=%s' % options.aapt2]
    cmd_args += ['--bundle=%s' % options.bundle]

    cmd_args += ['--output=%s' % tmp_bundle]
    if options.keystore_path:
      cmd_args += [
        '--ks=%s' % options.keystore_path,
        '--ks-key-alias=%s' % options.key_name,
        '--ks-pass=pass:%s' % options.keystore_password
      ]

    build_utils.CheckOutput(cmd_args)
    shutil.move(tmp_bundle, options.out_zip)


if __name__ == '__main__':
  main(sys.argv[1:])
