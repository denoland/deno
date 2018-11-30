# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging
import os
import sys

sys.path.append(os.path.join(os.path.dirname(__file__), '..', '..', 'gyp'))

from util import build_utils
from util import md5_check
import bundletool

def GenerateBundleApks(bundle_path, bundle_apks_path, aapt2_path,
                       keystore_path, keystore_password, keystore_alias,
                       universal):
  """Generate an .apks archive from a an app bundle if needed.

  Args:
    bundle_path: Input bundle file path.
    bundle_apks_path: Output bundle .apks archive path. Name must end with
      '.apks' or this operation will fail.
    aapt2_path: Path to aapt2 build tool.
    keystore_path: Path to keystore.
    keystore_password: Keystore password, as a string.
    keystore_alias: Keystore signing key alias.
    universal: Whether to create a single APK that contains the contents of all
      modules.
  """
  # NOTE: BUNDLETOOL_JAR_PATH is added to input_strings, rather than
  # input_paths, to speed up MD5 computations by about 400ms (the .jar file
  # contains thousands of class files which are checked independently,
  # resulting in an .md5.stamp of more than 60000 lines!).
  input_paths = [
      bundle_path,
      aapt2_path,
      keystore_path
  ]
  input_strings = [
      keystore_password,
      keystore_alias,
      bundletool.BUNDLETOOL_JAR_PATH,
      # NOTE: BUNDLETOOL_VERSION is already part of BUNDLETOOL_JAR_PATH, but
      # it's simpler to assume that this may not be the case in the future.
      bundletool.BUNDLETOOL_VERSION
  ]
  output_paths = [bundle_apks_path]

  def rebuild():
    logging.info('Building %s', os.path.basename(bundle_apks_path))
    with build_utils.AtomicOutput(bundle_apks_path) as tmp_apks:
      cmd_args = [
          'java', '-jar', bundletool.BUNDLETOOL_JAR_PATH, 'build-apks',
          '--aapt2=%s' % aapt2_path,
          '--output=%s' % tmp_apks.name,
          '--bundle=%s' % bundle_path,
          '--ks=%s' % keystore_path,
          '--ks-pass=pass:%s' % keystore_password,
          '--ks-key-alias=%s' % keystore_alias,
          '--overwrite',
      ]
      if universal:
        cmd_args += ['--universal']
      build_utils.CheckOutput(cmd_args)

  md5_check.CallAndRecordIfStale(
    rebuild,
    input_paths=input_paths,
    input_strings=input_strings,
    output_paths=output_paths)
