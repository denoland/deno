# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import os
import plist_util
import sys

# This script creates a PkgInfo file for an OS X .app bundle's plist.
# Usage: python write_pkg_info.py --plist Foo.app/Contents/Info.plist \
#           --output Foo.app/Contents/PkgInfo

def Main():
  parser = argparse.ArgumentParser(
      description='A script to write PkgInfo files for .app bundles.')
  parser.add_argument('--plist', required=True,
                      help='Path to the Info.plist for the .app.')
  parser.add_argument('--output', required=True,
                      help='Path to the desired output file.')
  args = parser.parse_args()

  # Remove the output if it exists already.
  if os.path.exists(args.output):
    os.unlink(args.output)

  plist = plist_util.LoadPList(args.plist)
  package_type = plist['CFBundlePackageType']
  if package_type != 'APPL':
    raise ValueError('Expected CFBundlePackageType to be %s, got %s' % \
        ('AAPL', package_type))

  # The format of PkgInfo is eight characters, representing the bundle type
  # and bundle signature, each four characters. If that is missing, four
  # '?' characters are used instead.
  signature_code = plist.get('CFBundleSignature', '????')
  if len(signature_code) != 4:
    raise ValueError('CFBundleSignature should be exactly four characters, ' +
        'got %s' % signature_code)

  with open(args.output, 'w') as fp:
    fp.write('%s%s' % (package_type, signature_code))
  return 0


if __name__ == '__main__':
  sys.exit(Main())
