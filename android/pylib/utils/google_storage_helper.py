# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Helper functions to upload data to Google Storage.

Text data should be streamed to logdog using |logdog_helper| module.
Due to logdog not having image or HTML viewer, those instead should be uploaded
to Google Storage directly using this module.
"""

import logging
import os
import sys
import time
import urlparse

from pylib.constants import host_paths
from pylib.utils import decorators

if host_paths.DEVIL_PATH not in sys.path:
  sys.path.append(host_paths.DEVIL_PATH)
from devil.utils import cmd_helper

_GSUTIL_PATH = os.path.join(
    host_paths.DIR_SOURCE_ROOT, 'third_party', 'catapult',
    'third_party', 'gsutil', 'gsutil.py')
_PUBLIC_URL = 'https://storage.googleapis.com/%s/'
_AUTHENTICATED_URL = 'https://storage.cloud.google.com/%s/'


@decorators.NoRaiseException(default_return_value='')
def upload(name, filepath, bucket, gs_args=None, command_args=None,
           content_type=None, authenticated_link=True):
  """Uploads data to Google Storage.

  Args:
    name: Name of the file on Google Storage.
    filepath: Path to file you want to upload.
    bucket: Bucket to upload file to.
    content_type: Content type to upload as. If not specified, Google storage
        will attempt to infer content type from file extension.
    authenticated_link: Whether to return a link that requires user to
        authenticate with a Google account. Setting this to false will return
        a link that does not require user to be signed into Google account but
        will only work for completely public storage buckets.
  Returns:
    Web link to item uploaded to Google Storage bucket.
  """
  bucket = _format_bucket_name(bucket)

  gs_path = 'gs://%s/%s' % (bucket, name)
  logging.info('Uploading %s to %s', filepath, gs_path)

  cmd = [_GSUTIL_PATH, '-q']
  cmd.extend(gs_args or [])
  if content_type:
    cmd.extend(['-h', 'Content-Type:%s' % content_type])
  cmd.extend(['cp'] + (command_args or []) + [filepath, gs_path])

  cmd_helper.RunCmd(cmd)

  return get_url_link(name, bucket, authenticated_link)


@decorators.NoRaiseException(default_return_value='')
def read_from_link(link):
  # Note that urlparse returns the path with an initial '/', so we only need to
  # add one more after the 'gs;'
  gs_path = 'gs:/%s' % urlparse.urlparse(link).path
  cmd = [_GSUTIL_PATH, '-q', 'cat', gs_path]
  return cmd_helper.GetCmdOutput(cmd)


@decorators.NoRaiseException(default_return_value=False)
def exists(name, bucket):
  bucket = _format_bucket_name(bucket)
  gs_path = 'gs://%s/%s' % (bucket, name)

  cmd = [_GSUTIL_PATH, '-q', 'stat', gs_path]
  return_code = cmd_helper.RunCmd(cmd)
  return return_code == 0


# TODO(jbudorick): Delete this function. Only one user of it.
def unique_name(basename, suffix='', timestamp=True, device=None):
  """Helper function for creating a unique name for a file to store in GS.

  Args:
    basename: Base of the unique filename.
    suffix: Suffix of filename.
    timestamp: Whether or not to add a timestamp to name.
    device: Device to add device serial of to name.
  """
  return '%s%s%s%s' % (
      basename,
      '_%s' % time.strftime('%Y_%m_%d_T%H_%M_%S-UTC', time.gmtime())
          if timestamp else '',
      '_%s' % device.serial if device else '',
      suffix)


def get_url_link(name, bucket, authenticated_link=True):
  """Get url link before/without uploading.

  Args:
    name: Name of the file on Google Storage.
    bucket: Bucket to upload file to.
    authenticated_link: Whether to return a link that requires user to
        authenticate with a Google account. Setting this to false will return
        a link that does not require user to be signed into Google account but
        will only work for completely public storage buckets.
  Returns:
    Web link to item to be uploaded to Google Storage bucket
  """
  bucket = _format_bucket_name(bucket)
  url_template = _AUTHENTICATED_URL if authenticated_link else _PUBLIC_URL
  return os.path.join(url_template % bucket, name)


def _format_bucket_name(bucket):
  if bucket.startswith('gs://'):
    bucket = bucket[len('gs://'):]
  if bucket.endswith('/'):
    bucket = bucket[:-1]
  return bucket
