#!/usr/bin/env python
#
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Install *_incremental.apk targets as well as their dependent files."""

import argparse
import glob
import json
import logging
import os
import posixpath
import shutil
import sys
import zipfile

sys.path.append(
    os.path.abspath(os.path.join(os.path.dirname(__file__), os.pardir)))
import devil_chromium
from devil.android import apk_helper
from devil.android import device_utils
from devil.android.sdk import version_codes
from devil.utils import reraiser_thread
from devil.utils import run_tests_helper
from pylib import constants
from pylib.utils import time_profile

prev_sys_path = list(sys.path)
sys.path.insert(0, os.path.join(os.path.dirname(__file__), os.pardir, 'gyp'))
from util import build_utils
sys.path = prev_sys_path


def _DeviceCachePath(device):
  file_name = 'device_cache_%s.json' % device.adb.GetDeviceSerial()
  return os.path.join(constants.GetOutDirectory(), file_name)


def _TransformDexPaths(paths):
  """Given paths like ["/a/b/c", "/a/c/d"], returns ["b.c", "c.d"]."""
  if len(paths) == 1:
    return [os.path.basename(paths[0])]

  prefix_len = len(os.path.commonprefix(paths))
  return [p[prefix_len:].replace(os.sep, '.') for p in paths]


def _Execute(concurrently, *funcs):
  """Calls all functions in |funcs| concurrently or in sequence."""
  timer = time_profile.TimeProfile()
  if concurrently:
    reraiser_thread.RunAsync(funcs)
  else:
    for f in funcs:
      f()
  timer.Stop(log=False)
  return timer


def _GetDeviceIncrementalDir(package):
  """Returns the device path to put incremental files for the given package."""
  return '/data/local/tmp/incremental-app-%s' % package


def _HasClasses(jar_path):
  """Returns whether the given jar contains classes.dex."""
  with zipfile.ZipFile(jar_path) as jar:
    return 'classes.dex' in jar.namelist()


def Uninstall(device, package, enable_device_cache=False):
  """Uninstalls and removes all incremental files for the given package."""
  main_timer = time_profile.TimeProfile()
  device.Uninstall(package)
  if enable_device_cache:
    # Uninstall is rare, so just wipe the cache in this case.
    cache_path = _DeviceCachePath(device)
    if os.path.exists(cache_path):
      os.unlink(cache_path)
  device.RunShellCommand(['rm', '-rf', _GetDeviceIncrementalDir(package)],
                         check_return=True)
  logging.info('Uninstall took %s seconds.', main_timer.GetDelta())


def Install(device, install_json, apk=None, enable_device_cache=False,
            use_concurrency=True, permissions=()):
  """Installs the given incremental apk and all required supporting files.

  Args:
    device: A DeviceUtils instance (to install to).
    install_json: Path to .json file or already parsed .json object.
    apk: An existing ApkHelper instance for the apk (optional).
    enable_device_cache: Whether to enable on-device caching of checksums.
    use_concurrency: Whether to speed things up using multiple threads.
    permissions: A list of the permissions to grant, or None to grant all
                 non-blacklisted permissions in the manifest.
  """
  if isinstance(install_json, basestring):
    with open(install_json) as f:
      install_dict = json.load(f)
  else:
    install_dict = install_json

  if install_dict.get('dont_even_try'):
    raise Exception(install_dict['dont_even_try'])

  main_timer = time_profile.TimeProfile()
  install_timer = time_profile.TimeProfile()
  push_native_timer = time_profile.TimeProfile()
  push_dex_timer = time_profile.TimeProfile()

  def fix_path(p):
    return os.path.normpath(os.path.join(constants.GetOutDirectory(), p))

  if not apk:
    apk = apk_helper.ToHelper(fix_path(install_dict['apk_path']))
  split_globs = [fix_path(p) for p in install_dict['split_globs']]
  native_libs = [fix_path(p) for p in install_dict['native_libs']]
  dex_files = [fix_path(p) for p in install_dict['dex_files']]
  show_proguard_warning = install_dict.get('show_proguard_warning')

  apk_package = apk.GetPackageName()
  device_incremental_dir = _GetDeviceIncrementalDir(apk_package)

  # Install .apk(s) if any of them have changed.
  def do_install():
    install_timer.Start()
    if split_globs:
      splits = []
      for split_glob in split_globs:
        splits.extend((f for f in glob.glob(split_glob)))
      device.InstallSplitApk(apk, splits, reinstall=True,
                             allow_cached_props=True, permissions=permissions)
    else:
      device.Install(apk, reinstall=True, permissions=permissions)
    install_timer.Stop(log=False)

  # Push .so and .dex files to the device (if they have changed).
  def do_push_files():
    push_native_timer.Start()
    if native_libs:
      with build_utils.TempDir() as temp_dir:
        device_lib_dir = posixpath.join(device_incremental_dir, 'lib')
        for path in native_libs:
          # Note: Can't use symlinks as they don't work when
          # "adb push parent_dir" is used (like we do here).
          shutil.copy(path, os.path.join(temp_dir, os.path.basename(path)))
        device.PushChangedFiles([(temp_dir, device_lib_dir)],
                                delete_device_stale=True)
    push_native_timer.Stop(log=False)

    push_dex_timer.Start()
    if dex_files:
      # Put all .dex files to be pushed into a temporary directory so that we
      # can use delete_device_stale=True.
      with build_utils.TempDir() as temp_dir:
        device_dex_dir = posixpath.join(device_incremental_dir, 'dex')
        # Ensure no two files have the same name.
        transformed_names = _TransformDexPaths(dex_files)
        for src_path, dest_name in zip(dex_files, transformed_names):
          # Binary targets with no extra classes create .dex.jar without a
          # classes.dex (which Android chokes on).
          if _HasClasses(src_path):
            shutil.copy(src_path, os.path.join(temp_dir, dest_name))
        device.PushChangedFiles([(temp_dir, device_dex_dir)],
                                delete_device_stale=True)
    push_dex_timer.Stop(log=False)

  def check_selinux():
    # Marshmallow has no filesystem access whatsoever. It might be possible to
    # get things working on Lollipop, but attempts so far have failed.
    # http://crbug.com/558818
    has_selinux = device.build_version_sdk >= version_codes.LOLLIPOP
    if has_selinux and apk.HasIsolatedProcesses():
      raise Exception('Cannot use incremental installs on Android L+ without '
                      'first disabling isolated processes.\n'
                      'To do so, use GN arg:\n'
                      '    disable_incremental_isolated_processes=true')

  cache_path = _DeviceCachePath(device)
  def restore_cache():
    if not enable_device_cache:
      return
    if os.path.exists(cache_path):
      logging.info('Using device cache: %s', cache_path)
      with open(cache_path) as f:
        device.LoadCacheData(f.read())
      # Delete the cached file so that any exceptions cause it to be cleared.
      os.unlink(cache_path)
    else:
      logging.info('No device cache present: %s', cache_path)

  def save_cache():
    if not enable_device_cache:
      return
    with open(cache_path, 'w') as f:
      f.write(device.DumpCacheData())
      logging.info('Wrote device cache: %s', cache_path)

  # Create 2 lock files:
  # * install.lock tells the app to pause on start-up (until we release it).
  # * firstrun.lock is used by the app to pause all secondary processes until
  #   the primary process finishes loading the .dex / .so files.
  def create_lock_files():
    # Creates or zeros out lock files.
    cmd = ('D="%s";'
           'mkdir -p $D &&'
           'echo -n >$D/install.lock 2>$D/firstrun.lock')
    device.RunShellCommand(
        cmd % device_incremental_dir, shell=True, check_return=True)

  # The firstrun.lock is released by the app itself.
  def release_installer_lock():
    device.RunShellCommand('echo > %s/install.lock' % device_incremental_dir,
                           check_return=True, shell=True)

  # Concurrency here speeds things up quite a bit, but DeviceUtils hasn't
  # been designed for multi-threading. Enabling only because this is a
  # developer-only tool.
  setup_timer = _Execute(
      use_concurrency, create_lock_files, restore_cache, check_selinux)

  _Execute(use_concurrency, do_install, do_push_files)

  finalize_timer = _Execute(use_concurrency, release_installer_lock, save_cache)

  logging.info(
      'Install of %s took %s seconds '
      '(setup=%s, install=%s, libs=%s, dex=%s, finalize=%s)',
      os.path.basename(apk.path), main_timer.GetDelta(), setup_timer.GetDelta(),
      install_timer.GetDelta(), push_native_timer.GetDelta(),
      push_dex_timer.GetDelta(), finalize_timer.GetDelta())
  if show_proguard_warning:
    logging.warning('Target had proguard enabled, but incremental install uses '
                    'non-proguarded .dex files. Performance characteristics '
                    'may differ.')


def main():
  parser = argparse.ArgumentParser()
  parser.add_argument('json_path',
                      help='The path to the generated incremental apk .json.')
  parser.add_argument('-d', '--device', dest='device',
                      help='Target device for apk to install on.')
  parser.add_argument('--uninstall',
                      action='store_true',
                      default=False,
                      help='Remove the app and all side-loaded files.')
  parser.add_argument('--output-directory',
                      help='Path to the root build directory.')
  parser.add_argument('--no-threading',
                      action='store_false',
                      default=True,
                      dest='threading',
                      help='Do not install and push concurrently')
  parser.add_argument('--no-cache',
                      action='store_false',
                      default=True,
                      dest='cache',
                      help='Do not use cached information about what files are '
                           'currently on the target device.')
  parser.add_argument('-v',
                      '--verbose',
                      dest='verbose_count',
                      default=0,
                      action='count',
                      help='Verbose level (multiple times for more)')

  args = parser.parse_args()

  run_tests_helper.SetLogLevel(args.verbose_count)
  if args.output_directory:
    constants.SetOutputDirectory(args.output_directory)

  devil_chromium.Initialize(output_directory=constants.GetOutDirectory())

  # Retries are annoying when commands fail for legitimate reasons. Might want
  # to enable them if this is ever used on bots though.
  device = device_utils.DeviceUtils.HealthyDevices(
      device_arg=args.device,
      default_retries=0,
      enable_device_files_cache=True)[0]

  if args.uninstall:
    with open(args.json_path) as f:
      install_dict = json.load(f)
    apk = apk_helper.ToHelper(install_dict['apk_path'])
    Uninstall(device, apk.GetPackageName(), enable_device_cache=args.cache)
  else:
    Install(device, args.json_path, enable_device_cache=args.cache,
            use_concurrency=args.threading)


if __name__ == '__main__':
  sys.exit(main())
