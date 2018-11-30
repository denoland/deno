#!/usr/bin/env python
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""
Uploads or downloads third party libraries to or from google cloud storage.

This script will only work for Android checkouts.
"""

import argparse
import logging
import os
import sys


sys.path.append(os.path.abspath(
    os.path.join(os.path.dirname(__file__), os.pardir)))
from pylib import constants
from pylib.constants import host_paths

sys.path.append(os.path.abspath(
    os.path.join(host_paths.DIR_SOURCE_ROOT, 'build')))
import find_depot_tools  # pylint: disable=import-error,unused-import
import download_from_google_storage
import upload_to_google_storage


def _AddBasicArguments(parser):
  parser.add_argument(
      '--sdk-root', default=constants.ANDROID_SDK_ROOT,
      help='base path to the Android SDK root')
  parser.add_argument(
      '-v', '--verbose', action='store_true', help='print debug information')
  parser.add_argument(
      '-b', '--bucket-path', required=True,
      help='The path of the lib file in Google Cloud Storage.')
  parser.add_argument(
      '-l', '--local-path', required=True,
      help='The base path of the third_party directory')


def _CheckPaths(bucket_path, local_path):
  if bucket_path.startswith('gs://'):
    bucket_url = bucket_path
  else:
    bucket_url = 'gs://%s' % bucket_path
  local_path = os.path.join(host_paths.DIR_SOURCE_ROOT, local_path)
  if not os.path.isdir(local_path):
    raise IOError(
        'The library local path is not a valid directory: %s' % local_path)
  return bucket_url, local_path


def _CheckFileList(local_path, file_list):
  local_path = os.path.abspath(local_path)
  abs_path_list = [os.path.abspath(f) for f in file_list]
  for f in abs_path_list:
    if os.path.commonprefix([f, local_path]) != local_path:
      raise IOError(
          '%s in the arguments is not descendant of the specified directory %s'
          % (f, local_path))
  return abs_path_list


def _PurgeSymlinks(local_path):
  for dirpath, _, filenames in os.walk(local_path):
    for f in filenames:
      path = os.path.join(dirpath, f)
      if os.path.islink(path):
        os.remove(path)


def Upload(arguments):
  """Upload files in a third_party directory to google storage"""
  bucket_url, local_path = _CheckPaths(arguments.bucket_path,
                                       arguments.local_path)
  file_list = _CheckFileList(local_path, arguments.file_list)
  return upload_to_google_storage.upload_to_google_storage(
      input_filenames=file_list,
      base_url=bucket_url,
      gsutil=arguments.gsutil,
      force=False,
      use_md5=False,
      num_threads=1,
      skip_hashing=False,
      gzip=None)


def Download(arguments):
  """Download files based on sha1 files in a third_party dir from gcs"""
  bucket_url, local_path = _CheckPaths(arguments.bucket_path,
                                       arguments.local_path)
  _PurgeSymlinks(local_path)
  return download_from_google_storage.download_from_google_storage(
      local_path,
      bucket_url,
      gsutil=arguments.gsutil,
      num_threads=1,
      directory=True,
      recursive=True,
      force=False,
      output=None,
      ignore_errors=False,
      sha1_file=None,
      verbose=arguments.verbose,
      auto_platform=False,
      extract=False)


def main(argv):
  parser = argparse.ArgumentParser()
  subparsers = parser.add_subparsers(title='commands')
  download_parser = subparsers.add_parser(
      'download', help='download the library from the cloud storage')
  _AddBasicArguments(download_parser)
  download_parser.set_defaults(func=Download)

  upload_parser = subparsers.add_parser(
      'upload', help='find all jar files in a third_party directory and ' +
                     'upload them to cloud storage')
  _AddBasicArguments(upload_parser)
  upload_parser.set_defaults(func=Upload)
  upload_parser.add_argument(
      '-f', '--file-list', nargs='+', required=True,
      help='A list of base paths for files in third_party to upload.')

  arguments = parser.parse_args(argv)
  if not os.path.isdir(arguments.sdk_root):
    logging.debug('Did not find the Android SDK root directory at "%s".',
                  arguments.sdk_root)
    logging.info('Skipping, not on an android checkout.')
    return 0

  arguments.gsutil = download_from_google_storage.Gsutil(
      download_from_google_storage.GSUTIL_DEFAULT_PATH)
  return arguments.func(arguments)


if __name__ == '__main__':
  sys.exit(main(sys.argv[1:]))
