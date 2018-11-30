#!/usr/bin/env python
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# Script to install the Chrome OS fonts on Linux.
# This script can be run manually (as root), but is also run as part
# install-build-deps.sh.

import os
import shutil
import subprocess
import sys

URL_TEMPLATE = ('https://commondatastorage.googleapis.com/chromeos-localmirror/'
                'distfiles/%(name)s-%(version)s.tar.bz2')

# Taken from the media-fonts/<name> ebuilds in chromiumos-overlay.
# noto-cjk used to be here, but is removed because fc-cache takes too long
# regenerating the fontconfig cache (See crbug.com/697954.)
# TODO(jshin): Add it back when the above issue can be avoided.
SOURCES = [
  {
    'name': 'notofonts',
    'version': '20161129'
  }, {
    'name': 'robotofonts',
    'version': '2.132'
  }
]

URLS = sorted([URL_TEMPLATE % d for d in SOURCES])
FONTS_DIR = '/usr/local/share/fonts'

def main(args):
  if not sys.platform.startswith('linux'):
    print "Error: %s must be run on Linux." % __file__
    return 1

  if os.getuid() != 0:
    print "Error: %s must be run as root." % __file__
    return 1

  if not os.path.isdir(FONTS_DIR):
    print "Error: Destination directory does not exist: %s" % FONTS_DIR
    return 1

  dest_dir = os.path.join(FONTS_DIR, 'chromeos')

  stamp = os.path.join(dest_dir, ".stamp02")
  if os.path.exists(stamp):
    with open(stamp) as s:
      if s.read() == '\n'.join(URLS):
        print "Chrome OS fonts already up to date in %s." % dest_dir
        return 0

  if os.path.isdir(dest_dir):
    shutil.rmtree(dest_dir)
  os.mkdir(dest_dir)
  os.chmod(dest_dir, 0755)

  print "Installing Chrome OS fonts to %s." % dest_dir
  for url in URLS:
    tarball = os.path.join(dest_dir, os.path.basename(url))
    subprocess.check_call(['curl', '-L', url, '-o', tarball])
    subprocess.check_call(['tar', '--no-same-owner', '--no-same-permissions',
                           '-xf', tarball, '-C', dest_dir])
    os.remove(tarball)

  readme = os.path.join(dest_dir, "README")
  with open(readme, 'w') as s:
    s.write("This directory and its contents are auto-generated.\n")
    s.write("It may be deleted and recreated. Do not modify.\n")
    s.write("Script: %s\n" % __file__)

  with open(stamp, 'w') as s:
    s.write('\n'.join(URLS))

  for base, dirs, files in os.walk(dest_dir):
    for dir in dirs:
      os.chmod(os.path.join(base, dir), 0755)
    for file in files:
      os.chmod(os.path.join(base, file), 0644)

  print """\

Chrome OS font rendering settings are specified using Fontconfig. If your
system's configuration doesn't match Chrome OS's (which vary for different
devices), fonts may be rendered with different subpixel rendering, subpixel
positioning, or hinting settings. This may affect font metrics.

Chrome OS's settings are stored in the media-libs/fontconfig package, which is
at src/third_party/chromiumos-overlay/media-libs/fontconfig in a Chrome OS
checkout. You can configure your system to match Chrome OS's defaults by
creating or editing a ~/.fonts.conf file:

<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig>
  <match target="font">
    <edit name="antialias" mode="assign"><bool>true</bool></edit>
    <edit name="autohint" mode="assign"><bool>true</bool></edit>
    <edit name="hinting" mode="assign"><bool>true</bool></edit>
    <edit name="hintstyle" mode="assign"><const>hintslight</const></edit>
    <edit name="rgba" mode="assign"><const>rgb</const></edit>
  </match>
</fontconfig>

To load additional per-font configs (and assuming you have Chrome OS checked
out), add the following immediately before the "</fontconfig>" line:

  <include ignore_missing="yes">/path/to/src/third_party/chromiumos-overlay/media-libs/fontconfig/files/local.conf</include>
"""

  return 0

if __name__ == '__main__':
  sys.exit(main(sys.argv[1:]))
