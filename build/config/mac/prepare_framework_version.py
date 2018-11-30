# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
import shutil
import sys

# Ensures that the current version matches the last-produced version, which is
# stored in the version_file. If it does not, then the framework_root_dir is
# obliterated.
# Usage: python prepare_framework_version.py out/obj/version_file \
#                                            out/Framework.framework \
#                                            'A'

def PrepareFrameworkVersion(version_file, framework_root_dir, version):
  # Test what the current framework version is. Stop if it is up-to-date.
  try:
    with open(version_file, 'r') as f:
      current_version = f.read()
      if current_version == version:
        return
  except IOError:
    pass

  # The framework version has changed, so clobber the framework.
  if os.path.exists(framework_root_dir):
    shutil.rmtree(framework_root_dir)

  # Write out the new framework version file, making sure its containing
  # directory exists.
  dirname = os.path.dirname(version_file)
  if not os.path.isdir(dirname):
    os.makedirs(dirname, 0700)

  with open(version_file, 'w+') as f:
    f.write(version)


if __name__ == '__main__':
  PrepareFrameworkVersion(sys.argv[1], sys.argv[2], sys.argv[3])
  sys.exit(0)
