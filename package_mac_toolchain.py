#!/usr/bin/env python
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Compress and upload Mac toolchain files.

Stored in in https://pantheon.corp.google.com/storage/browser/chrome-mac-sdk/.
"""

import argparse
import glob
import os
import plistlib
import re
import subprocess
import sys
import tarfile
import tempfile


TOOLCHAIN_URL = "gs://chrome-mac-sdk"

# It's important to at least remove unused Platform folders to cut down on the
# size of the toolchain folder.  There are other various unused folders that
# have been removed through trial and error.  If future versions of Xcode become
# problematic it's possible this list is incorrect, and can be reduced to just
# the unused platforms.  On the flip side, it's likely more directories can be
# excluded.
DEFAULT_EXCLUDE_FOLDERS = [
'Contents/Applications',
'Contents/Developer/Documentation',
'Contents/Developer/Library/Xcode/Templates',
'Contents/Developer/Platforms/AppleTVOS.platform',
'Contents/Developer/Platforms/AppleTVSimulator.platform',
'Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/'
    'usr/share/man/',
'Contents/Developer/Platforms/WatchOS.platform',
'Contents/Developer/Platforms/WatchSimulator.platform',
'Contents/Developer/Toolchains/Swift*',
'Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift',
'Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-migrator',
'Contents/Resources/Packages/MobileDevice.pkg',
]

MAC_EXCLUDE_FOLDERS = [
# The only thing we need in iPhoneOS.platform on mac is:
#  \Developer\Library\Xcode\PrivatePlugins
#  \Info.Plist.
#  This is the cleanest way to get these.
'Contents/Developer/Platforms/iPhoneOS.platform/Developer/Library/Frameworks',
'Contents/Developer/Platforms/iPhoneOS.platform/Developer/Library/GPUTools',
'Contents/Developer/Platforms/iPhoneOS.platform/Developer/Library/'
    'GPUToolsPlatform',
'Contents/Developer/Platforms/iPhoneOS.platform/Developer/Library/'
    'PrivateFrameworks',
'Contents/Developer/Platforms/iPhoneOS.platform/Developer/usr',
'Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs',
'Contents/Developer/Platforms/iPhoneOS.platform/DeviceSupport',
'Contents/Developer/Platforms/iPhoneOS.platform/Library',
'Contents/Developer/Platforms/iPhoneOS.platform/usr',

# iPhoneSimulator has a similar requirement, but the bulk of the binary size is
# in \Developer\SDKs, so only excluding that here.
'Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs',
]

IOS_EXCLUDE_FOLDERS = [
'Contents/Developer/Platforms/iPhoneOS.platform/DeviceSupport/'
'Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/'
    'iPhoneSimulator.sdk/Applications/',
'Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/'
    'iPhoneSimulator.sdk/System/Library/AccessibilityBundles/',
'Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/'
    'iPhoneSimulator.sdk/System/Library/CoreServices/',
'Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/'
    'iPhoneSimulator.sdk/System/Library/LinguisticData/',
]

def main():
  """Compress |target_dir| and upload to |TOOLCHAIN_URL|"""
  parser = argparse.ArgumentParser()
  parser.add_argument('target_dir',
                      help="Xcode installation directory.")
  parser.add_argument('platform', choices=['ios', 'mac'],
                      help="Target platform for bundle.")
  parser_args = parser.parse_args()

  # Verify this looks like an Xcode directory.
  contents_dir = os.path.join(parser_args.target_dir, 'Contents')
  plist_file = os.path.join(contents_dir, 'version.plist')
  try:
    info = plistlib.readPlist(plist_file)
  except:
    print "Invalid Xcode dir."
    return 0
  build_version = info['ProductBuildVersion']

  # Look for previous toolchain tgz files with the same |build_version|.
  fname = 'toolchain'
  if parser_args.platform == 'ios':
    fname = 'ios-' + fname
  wildcard_filename = '%s/%s-%s-*.tgz' % (TOOLCHAIN_URL, fname, build_version)
  p = subprocess.Popen(['gsutil.py', 'ls', wildcard_filename],
                       stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE)
  output = p.communicate()[0]
  next_count = 1
  if p.returncode == 0:
    next_count = len(output.split('\n'))
    sys.stdout.write("%s already exists (%s). "
                     "Do you want to create another? [y/n] "
                     % (build_version, next_count - 1))

    if raw_input().lower() not in set(['yes','y', 'ye']):
      print "Skipping duplicate upload."
      return 0

  os.chdir(parser_args.target_dir)
  toolchain_file_name = "%s-%s-%s" % (fname, build_version, next_count)
  toolchain_name = tempfile.mktemp(suffix='toolchain.tgz')

  print "Creating %s (%s)." % (toolchain_file_name, toolchain_name)
  os.environ["COPYFILE_DISABLE"] = "1"
  os.environ["GZ_OPT"] = "-8"
  args = ['tar', '-cvzf', toolchain_name]
  exclude_folders = DEFAULT_EXCLUDE_FOLDERS
  if parser_args.platform == 'mac':
    exclude_folders += MAC_EXCLUDE_FOLDERS
  else:
    exclude_folders += IOS_EXCLUDE_FOLDERS
  args.extend(map('--exclude={0}'.format, exclude_folders))
  args.extend(['.'])
  subprocess.check_call(args)

  print "Uploading %s toolchain." % toolchain_file_name
  destination_path = '%s/%s.tgz' % (TOOLCHAIN_URL, toolchain_file_name)
  subprocess.check_call(['gsutil.py', 'cp', '-n', toolchain_name,
                         destination_path])

  print "Done with %s upload." % toolchain_file_name
  return 0

if __name__ == '__main__':
  sys.exit(main())
