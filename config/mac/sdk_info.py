# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import doctest
import itertools
import os
import subprocess
import sys

# This script prints information about the build system, the operating
# system and the iOS or Mac SDK (depending on the platform "iphonesimulator",
# "iphoneos" or "macosx" generally).

def SplitVersion(version):
  """Splits the Xcode version to 3 values.

  >>> list(SplitVersion('8.2.1.1'))
  ['8', '2', '1']
  >>> list(SplitVersion('9.3'))
  ['9', '3', '0']
  >>> list(SplitVersion('10.0'))
  ['10', '0', '0']
  """
  version = version.split('.')
  return itertools.islice(itertools.chain(version, itertools.repeat('0')), 0, 3)

def FormatVersion(version):
  """Converts Xcode version to a format required for DTXcode in Info.plist

  >>> FormatVersion('8.2.1')
  '0821'
  >>> FormatVersion('9.3')
  '0930'
  >>> FormatVersion('10.0')
  '1000'
  """
  major, minor, patch = SplitVersion(version)
  return ('%2s%s%s' % (major, minor, patch)).replace(' ', '0')

def FillXcodeVersion(settings):
  """Fills the Xcode version and build number into |settings|."""
  lines = subprocess.check_output(['xcodebuild', '-version']).splitlines()
  settings['xcode_version'] = FormatVersion(lines[0].split()[-1])
  settings['xcode_version_int'] = int(settings['xcode_version'], 10)
  settings['xcode_build'] = lines[-1].split()[-1]


def FillMachineOSBuild(settings):
  """Fills OS build number into |settings|."""
  settings['machine_os_build'] = subprocess.check_output(
      ['sw_vers', '-buildVersion']).strip()


def FillSDKPathAndVersion(settings, platform, xcode_version):
  """Fills the SDK path and version for |platform| into |settings|."""
  settings['sdk_path'] = subprocess.check_output([
      'xcrun', '-sdk', platform, '--show-sdk-path']).strip()
  settings['sdk_version'] = subprocess.check_output([
      'xcrun', '-sdk', platform, '--show-sdk-version']).strip()
  settings['sdk_platform_path'] = subprocess.check_output([
      'xcrun', '-sdk', platform, '--show-sdk-platform-path']).strip()
  # TODO: unconditionally use --show-sdk-build-version once Xcode 7.2 or
  # higher is required to build Chrome for iOS or OS X.
  if xcode_version >= '0720':
    settings['sdk_build'] = subprocess.check_output([
        'xcrun', '-sdk', platform, '--show-sdk-build-version']).strip()
  else:
    settings['sdk_build'] = settings['sdk_version']


if __name__ == '__main__':
  doctest.testmod()

  parser = argparse.ArgumentParser()
  parser.add_argument("--developer_dir", required=False)
  args, unknownargs = parser.parse_known_args()
  if args.developer_dir:
    os.environ['DEVELOPER_DIR'] = args.developer_dir

  if len(unknownargs) != 1:
    sys.stderr.write(
        'usage: %s [iphoneos|iphonesimulator|macosx]\n' %
        os.path.basename(sys.argv[0]))
    sys.exit(1)

  settings = {}
  FillMachineOSBuild(settings)
  FillXcodeVersion(settings)
  FillSDKPathAndVersion(settings, unknownargs[0], settings['xcode_version'])

  for key in sorted(settings):
    value = settings[key]
    if isinstance(value, str):
      value = '"%s"' % value
    print '%s=%s' % (key, value)
