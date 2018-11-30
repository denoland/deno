#!/usr/bin/env python
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Utility for reading / writing command-line flag files on device(s)."""

import argparse
import os
import sys

import devil_chromium

from devil.android import device_utils
from devil.android import flag_changer
from devil.utils import cmd_helper


def main():
  parser = argparse.ArgumentParser(description=__doc__)
  parser.usage = '''%(prog)s --name FILENAME [--device SERIAL] [flags...]

No flags: Prints existing command-line file.
Empty string: Deletes command-line file.
Otherwise: Writes command-line file.

'''
  parser.add_argument('-d', '--device', dest='devices', action='append',
                      default=[], help='Target device serial (repeatable).')
  parser.add_argument('--name', required=True,
                      help='Name of file where to store flags on the device.')
  parser.add_argument('-e', '--executable', dest='executable', default='chrome',
                      help='(deprecated) No longer used.')
  parser.add_argument('--adb-path', type=os.path.abspath,
                      help='Path to the adb binary.')
  args, remote_args = parser.parse_known_args()

  devil_chromium.Initialize(adb_path=args.adb_path)

  devices = device_utils.DeviceUtils.HealthyDevices(device_arg=args.devices,
                                                    default_retries=0)
  all_devices = device_utils.DeviceUtils.parallel(devices)

  if not remote_args:
    # No args == do not update, just print flags.
    remote_args = None
    action = ''
  elif len(remote_args) == 1 and not remote_args[0]:
    # Single empty string arg == delete flags
    remote_args = []
    action = 'Deleted command line file. '
  else:
    action = 'Wrote command line file. '

  def update_flags(device):
    changer = flag_changer.FlagChanger(device, args.name)
    if remote_args is not None:
      flags = changer.ReplaceFlags(remote_args)
    else:
      flags = changer.GetCurrentFlags()
    return (device, device.build_description, flags)

  updated_values = all_devices.pMap(update_flags).pGet(None)

  print '%sCurrent flags (in %s):' % (action, args.name)
  for d, desc, flags in updated_values:
    if flags:
      # Shell-quote flags for easy copy/paste as new args on the terminal.
      quoted_flags = ' '.join(cmd_helper.SingleQuote(f) for f in sorted(flags))
    else:
      quoted_flags = '( empty )'
    print '  %s (%s): %s' % (d, desc, quoted_flags)

  return 0


if __name__ == '__main__':
  sys.exit(main())
