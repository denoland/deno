#!/usr/bin/env python
#
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Manually applies a shared preference JSON file.

If needed during automation, use the --shared-prefs-file in test_runner.py
instead.
"""

import argparse
import sys

# pylint: disable=ungrouped-imports
from pylib.constants import host_paths
if host_paths.DEVIL_PATH not in sys.path:
  sys.path.append(host_paths.DEVIL_PATH)

from devil.android import device_utils
from devil.android.sdk import shared_prefs
from pylib.utils import shared_preference_utils


def main():
  parser = argparse.ArgumentParser(
      description='Manually apply shared preference JSON files.')
  parser.add_argument('filepaths', nargs='*',
                      help='Any number of paths to shared preference JSON '
                           'files to apply.')
  args = parser.parse_args()

  all_devices = device_utils.DeviceUtils.HealthyDevices()
  if not all_devices:
    raise RuntimeError('No healthy devices attached')

  for filepath in args.filepaths:
    all_settings = shared_preference_utils.ExtractSettingsFromJson(filepath)
    for setting in all_settings:
      for device in all_devices:
        shared_pref = shared_prefs.SharedPrefs(
            device, setting['package'], setting['filename'],
            use_encrypted_path=setting.get('supports_encrypted_path', False))
        shared_preference_utils.ApplySharedPreferenceSetting(
            shared_pref, setting)


if __name__ == '__main__':
  main()
