#!/usr/bin/env python
#
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
#
# Find the most recent tombstone file(s) on all connected devices
# and prints their stacks.
#
# Assumes tombstone file was created with current symbols.

import argparse
import datetime
import logging
import os
import sys

from multiprocessing.pool import ThreadPool

import devil_chromium

from devil.android import device_blacklist
from devil.android import device_errors
from devil.android import device_utils
from devil.utils import run_tests_helper
from pylib import constants
from pylib.symbols import stack_symbolizer


_TZ_UTC = {'TZ': 'UTC'}


def _ListTombstones(device):
  """List the tombstone files on the device.

  Args:
    device: An instance of DeviceUtils.

  Yields:
    Tuples of (tombstone filename, date time of file on device).
  """
  try:
    if not device.PathExists('/data/tombstones', as_root=True):
      return
    entries = device.StatDirectory('/data/tombstones', as_root=True)
    for entry in entries:
      if 'tombstone' in entry['filename']:
        yield (entry['filename'],
               datetime.datetime.fromtimestamp(entry['st_mtime']))
  except device_errors.CommandFailedError:
    logging.exception('Could not retrieve tombstones.')
  except device_errors.DeviceUnreachableError:
    logging.exception('Device unreachable retrieving tombstones.')
  except device_errors.CommandTimeoutError:
    logging.exception('Timed out retrieving tombstones.')


def _GetDeviceDateTime(device):
  """Determine the date time on the device.

  Args:
    device: An instance of DeviceUtils.

  Returns:
    A datetime instance.
  """
  device_now_string = device.RunShellCommand(
      ['date'], check_return=True, env=_TZ_UTC)
  return datetime.datetime.strptime(
      device_now_string[0], '%a %b %d %H:%M:%S %Z %Y')


def _GetTombstoneData(device, tombstone_file):
  """Retrieve the tombstone data from the device

  Args:
    device: An instance of DeviceUtils.
    tombstone_file: the tombstone to retrieve

  Returns:
    A list of lines
  """
  return device.ReadFile(
      '/data/tombstones/' + tombstone_file, as_root=True).splitlines()


def _EraseTombstone(device, tombstone_file):
  """Deletes a tombstone from the device.

  Args:
    device: An instance of DeviceUtils.
    tombstone_file: the tombstone to delete.
  """
  return device.RunShellCommand(
      ['rm', '/data/tombstones/' + tombstone_file],
      as_root=True, check_return=True)


def _ResolveTombstone(args):
  tombstone = args[0]
  tombstone_symbolizer = args[1]
  lines = []
  lines += [tombstone['file'] + ' created on ' + str(tombstone['time']) +
            ', about this long ago: ' +
            (str(tombstone['device_now'] - tombstone['time']) +
            ' Device: ' + tombstone['serial'])]
  logging.info('\n'.join(lines))
  logging.info('Resolving...')
  lines += tombstone_symbolizer.ExtractAndResolveNativeStackTraces(
      tombstone['data'],
      tombstone['device_abi'],
      tombstone['stack'])
  return lines


def _ResolveTombstones(jobs, tombstones, tombstone_symbolizer):
  """Resolve a list of tombstones.

  Args:
    jobs: the number of jobs to use with multithread.
    tombstones: a list of tombstones.
  """
  if not tombstones:
    logging.warning('No tombstones to resolve.')
    return []
  if len(tombstones) == 1:
    data = [_ResolveTombstone([tombstones[0], tombstone_symbolizer])]
  else:
    pool = ThreadPool(jobs)
    data = pool.map(
        _ResolveTombstone,
        [[tombstone, tombstone_symbolizer] for tombstone in tombstones])
    pool.close()
    pool.join()
  resolved_tombstones = []
  for tombstone in data:
    resolved_tombstones.extend(tombstone)
  return resolved_tombstones


def _GetTombstonesForDevice(device, resolve_all_tombstones,
                            include_stack_symbols,
                            wipe_tombstones):
  """Returns a list of tombstones on a given device.

  Args:
    device: An instance of DeviceUtils.
    resolve_all_tombstone: Whether to resolve every tombstone.
    include_stack_symbols: Whether to include symbols for stack data.
    wipe_tombstones: Whether to wipe tombstones.
  """
  ret = []
  all_tombstones = list(_ListTombstones(device))
  if not all_tombstones:
    logging.warning('No tombstones.')
    return ret

  # Sort the tombstones in date order, descending
  all_tombstones.sort(cmp=lambda a, b: cmp(b[1], a[1]))

  # Only resolve the most recent unless --all-tombstones given.
  tombstones = all_tombstones if resolve_all_tombstones else [all_tombstones[0]]

  device_now = _GetDeviceDateTime(device)
  try:
    for tombstone_file, tombstone_time in tombstones:
      ret += [{'serial': str(device),
               'device_abi': device.product_cpu_abi,
               'device_now': device_now,
               'time': tombstone_time,
               'file': tombstone_file,
               'stack': include_stack_symbols,
               'data': _GetTombstoneData(device, tombstone_file)}]
  except device_errors.CommandFailedError:
    for entry in device.StatDirectory(
        '/data/tombstones', as_root=True, timeout=60):
      logging.info('%s: %s', str(device), entry)
    raise

  # Erase all the tombstones if desired.
  if wipe_tombstones:
    for tombstone_file, _ in all_tombstones:
      _EraseTombstone(device, tombstone_file)

  return ret


def ClearAllTombstones(device):
  """Clear all tombstones in the device.

  Args:
    device: An instance of DeviceUtils.
  """
  all_tombstones = list(_ListTombstones(device))
  if not all_tombstones:
    logging.warning('No tombstones to clear.')

  for tombstone_file, _ in all_tombstones:
    _EraseTombstone(device, tombstone_file)


def ResolveTombstones(device, resolve_all_tombstones, include_stack_symbols,
                      wipe_tombstones, jobs=4, apk_under_test=None,
                      tombstone_symbolizer=None):
  """Resolve tombstones in the device.

  Args:
    device: An instance of DeviceUtils.
    resolve_all_tombstone: Whether to resolve every tombstone.
    include_stack_symbols: Whether to include symbols for stack data.
    wipe_tombstones: Whether to wipe tombstones.
    jobs: Number of jobs to use when processing multiple crash stacks.

  Returns:
    A list of resolved tombstones.
  """
  return _ResolveTombstones(jobs,
                            _GetTombstonesForDevice(device,
                                                    resolve_all_tombstones,
                                                    include_stack_symbols,
                                                    wipe_tombstones),
                            (tombstone_symbolizer
                             or stack_symbolizer.Symbolizer(apk_under_test)))


def main():
  custom_handler = logging.StreamHandler(sys.stdout)
  custom_handler.setFormatter(run_tests_helper.CustomFormatter())
  logging.getLogger().addHandler(custom_handler)
  logging.getLogger().setLevel(logging.INFO)

  parser = argparse.ArgumentParser()
  parser.add_argument('--device',
                      help='The serial number of the device. If not specified '
                           'will use all devices.')
  parser.add_argument('--blacklist-file', help='Device blacklist JSON file.')
  parser.add_argument('-a', '--all-tombstones', action='store_true',
                      help='Resolve symbols for all tombstones, rather than '
                           'just the most recent.')
  parser.add_argument('-s', '--stack', action='store_true',
                      help='Also include symbols for stack data')
  parser.add_argument('-w', '--wipe-tombstones', action='store_true',
                      help='Erase all tombstones from device after processing')
  parser.add_argument('-j', '--jobs', type=int,
                      default=4,
                      help='Number of jobs to use when processing multiple '
                           'crash stacks.')
  parser.add_argument('--output-directory',
                      help='Path to the root build directory.')
  parser.add_argument('--adb-path', type=os.path.abspath,
                      help='Path to the adb binary.')
  args = parser.parse_args()

  devil_chromium.Initialize(adb_path=args.adb_path)

  blacklist = (device_blacklist.Blacklist(args.blacklist_file)
               if args.blacklist_file
               else None)

  if args.output_directory:
    constants.SetOutputDirectory(args.output_directory)
  # Do an up-front test that the output directory is known.
  constants.CheckOutputDirectory()

  if args.device:
    devices = [device_utils.DeviceUtils(args.device)]
  else:
    devices = device_utils.DeviceUtils.HealthyDevices(blacklist)

  # This must be done serially because strptime can hit a race condition if
  # used for the first time in a multithreaded environment.
  # http://bugs.python.org/issue7980
  for device in devices:
    resolved_tombstones = ResolveTombstones(
        device, args.all_tombstones,
        args.stack, args.wipe_tombstones, args.jobs)
    for line in resolved_tombstones:
      logging.info(line)


if __name__ == '__main__':
  sys.exit(main())
