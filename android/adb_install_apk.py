#!/usr/bin/env python
#
# Copyright (c) 2012 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Utility script to install APKs from the command line quickly."""

import argparse
import glob
import logging
import os
import sys

import devil_chromium
from devil.android import apk_helper
from devil.android import device_blacklist
from devil.android import device_errors
from devil.android import device_utils
from devil.utils import run_tests_helper
from pylib import constants


def main():
  parser = argparse.ArgumentParser()

  apk_group = parser.add_mutually_exclusive_group(required=True)
  apk_group.add_argument('--apk', dest='apk_name',
                         help='DEPRECATED The name of the apk containing the'
                              ' application (with the .apk extension).')
  apk_group.add_argument('apk_path', nargs='?',
                         help='The path to the APK to install.')

  # TODO(jbudorick): Remove once no clients pass --apk_package
  parser.add_argument('--apk_package', help='DEPRECATED unused')
  parser.add_argument('--split',
                      action='append',
                      dest='splits',
                      help='A glob matching the apk splits. '
                           'Can be specified multiple times.')
  parser.add_argument('--keep_data',
                      action='store_true',
                      default=False,
                      help='Keep the package data when installing '
                           'the application.')
  parser.add_argument('--debug', action='store_const', const='Debug',
                      dest='build_type',
                      default=os.environ.get('BUILDTYPE', 'Debug'),
                      help='If set, run test suites under out/Debug. '
                           'Default is env var BUILDTYPE or Debug')
  parser.add_argument('--release', action='store_const', const='Release',
                      dest='build_type',
                      help='If set, run test suites under out/Release. '
                           'Default is env var BUILDTYPE or Debug.')
  parser.add_argument('-d', '--device', dest='devices', action='append',
                      default=[],
                      help='Target device for apk to install on. Enter multiple'
                           ' times for multiple devices.')
  parser.add_argument('--adb-path', type=os.path.abspath,
                      help='Absolute path to the adb binary to use.')
  parser.add_argument('--blacklist-file', help='Device blacklist JSON file.')
  parser.add_argument('-v', '--verbose', action='count',
                      help='Enable verbose logging.')
  parser.add_argument('--downgrade', action='store_true',
                      help='If set, allows downgrading of apk.')
  parser.add_argument('--timeout', type=int,
                      default=device_utils.DeviceUtils.INSTALL_DEFAULT_TIMEOUT,
                      help='Seconds to wait for APK installation. '
                           '(default: %(default)s)')

  args = parser.parse_args()

  run_tests_helper.SetLogLevel(args.verbose)
  constants.SetBuildType(args.build_type)

  devil_chromium.Initialize(
      output_directory=constants.GetOutDirectory(),
      adb_path=args.adb_path)

  apk = args.apk_path or args.apk_name
  if not apk.endswith('.apk'):
    apk += '.apk'
  if not os.path.exists(apk):
    apk = os.path.join(constants.GetOutDirectory(), 'apks', apk)
    if not os.path.exists(apk):
      parser.error('%s not found.' % apk)

  if args.splits:
    splits = []
    base_apk_package = apk_helper.ApkHelper(apk).GetPackageName()
    for split_glob in args.splits:
      apks = [f for f in glob.glob(split_glob) if f.endswith('.apk')]
      if not apks:
        logging.warning('No apks matched for %s.', split_glob)
      for f in apks:
        helper = apk_helper.ApkHelper(f)
        if (helper.GetPackageName() == base_apk_package
            and helper.GetSplitName()):
          splits.append(f)

  blacklist = (device_blacklist.Blacklist(args.blacklist_file)
               if args.blacklist_file
               else None)
  devices = device_utils.DeviceUtils.HealthyDevices(blacklist=blacklist,
                                                    device_arg=args.devices)

  def blacklisting_install(device):
    try:
      if args.splits:
        device.InstallSplitApk(apk, splits, reinstall=args.keep_data,
                               allow_downgrade=args.downgrade)
      else:
        device.Install(apk, reinstall=args.keep_data,
                       allow_downgrade=args.downgrade,
                       timeout=args.timeout)
    except (device_errors.CommandFailedError,
            device_errors.DeviceUnreachableError):
      logging.exception('Failed to install %s', apk)
      if blacklist:
        blacklist.Extend([str(device)], reason='install_failure')
        logging.warning('Blacklisting %s', str(device))
    except device_errors.CommandTimeoutError:
      logging.exception('Timed out while installing %s', apk)
      if blacklist:
        blacklist.Extend([str(device)], reason='install_timeout')
        logging.warning('Blacklisting %s', str(device))

  device_utils.DeviceUtils.parallel(devices).pMap(blacklisting_install)


if __name__ == '__main__':
  sys.exit(main())
