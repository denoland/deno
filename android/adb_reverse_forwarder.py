#!/usr/bin/env python
#
# Copyright (c) 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Command line tool for forwarding ports from a device to the host.

Allows an Android device to connect to services running on the host machine,
i.e., "adb forward" in reverse. Requires |host_forwarder| and |device_forwarder|
to be built.
"""

import argparse
import sys
import time

import devil_chromium

from devil.android import device_blacklist
from devil.android import device_utils
from devil.android import forwarder
from devil.utils import run_tests_helper

from pylib import constants


def main(argv):
  parser = argparse.ArgumentParser(
      usage='Usage: %(prog)s [options] device_port '
            'host_port [device_port_2 host_port_2] ...',
      description=__doc__)
  parser.add_argument(
      '-v', '--verbose',
      dest='verbose_count',
      default=0,
      action='count',
      help='Verbose level (multiple times for more)')
  parser.add_argument(
      '--device',
      help='Serial number of device we should use.')
  parser.add_argument(
      '--blacklist-file',
      help='Device blacklist JSON file.')
  parser.add_argument(
      '--debug',
      action='store_const',
      const='Debug',
      dest='build_type',
      default='Release',
      help='DEPRECATED: use --output-directory instead.')
  parser.add_argument(
      '--output-directory',
      help='Path to the root build directory.')
  parser.add_argument(
      'ports',
      nargs='+',
      type=int,
      help='Port pair to reverse forward.')

  args = parser.parse_args(argv)
  run_tests_helper.SetLogLevel(args.verbose_count)

  if len(args.ports) < 2 or len(args.ports) % 2:
    parser.error('Need even number of port pairs')

  port_pairs = zip(args.ports[::2], args.ports[1::2])

  if args.build_type:
    constants.SetBuildType(args.build_type)
  if args.output_directory:
    constants.SetOutputDirectory(args.output_directory)
  devil_chromium.Initialize(output_directory=constants.GetOutDirectory())

  blacklist = (device_blacklist.Blacklist(args.blacklist_file)
               if args.blacklist_file
               else None)
  device = device_utils.DeviceUtils.HealthyDevices(
      blacklist=blacklist, device_arg=args.device)[0]
  try:
    forwarder.Forwarder.Map(port_pairs, device)
    while True:
      time.sleep(60)
  except KeyboardInterrupt:
    sys.exit(0)
  finally:
    forwarder.Forwarder.UnmapAllDevicePorts(device)

if __name__ == '__main__':
  sys.exit(main(sys.argv[1:]))
