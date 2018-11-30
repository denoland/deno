# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
"""Signs and aligns an APK."""

import argparse
import shutil
import subprocess
import tempfile


def FinalizeApk(apksigner_path, zipalign_path, unsigned_apk_path,
                final_apk_path, key_path, key_passwd, key_name):
  # Use a tempfile so that Ctrl-C does not leave the file with a fresh mtime
  # and a corrupted state.
  with tempfile.NamedTemporaryFile() as staging_file:
    # v2 signing requires that zipalign happen first.
    subprocess.check_output([
        zipalign_path, '-p', '-f', '4',
        unsigned_apk_path, staging_file.name])
    subprocess.check_output([
        apksigner_path, 'sign',
        '--in', staging_file.name,
        '--out', staging_file.name,
        '--ks', key_path,
        '--ks-key-alias', key_name,
        '--ks-pass', 'pass:' + key_passwd,
        # Force SHA-1 (makes signing faster; insecure is fine for local builds).
        '--min-sdk-version', '1',
    ])
    shutil.move(staging_file.name, final_apk_path)
    staging_file.delete = False
