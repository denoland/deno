#!/usr/bin/env python
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

'''Unittests for update.py.

They set up a temporary directory that is used to mock a bucket, the directory
containing the configuration files and the android sdk directory.

Tests run the script with various inputs and check the status of the filesystem
'''

import contextlib
import logging
import os
import shutil
import sys
import tempfile
import unittest
import zipfile

sys.path.append(os.path.join(os.path.dirname(__file__), os.pardir))
from play_services import update
import devil_chromium  # pylint: disable=import-error,unused-import
from devil.utils import cmd_helper


class TestFunctions(unittest.TestCase):
  DEFAULT_CONFIG_VERSION = '1.2.3'
  DEFAULT_LICENSE = 'Default License'
  DEFAULT_ZIP_SHA1 = 'zip0and0filling0to0forty0chars0000000000'

  def __init__(self, *args, **kwargs):
    super(TestFunctions, self).__init__(*args, **kwargs)
    self.paths = None  # Initialized in SetUpWorkdir
    self.workdir = None  # Initialized in setUp

    # Uncomment for debug logs.
    # logging.basicConfig(level=logging.DEBUG)

  #override
  def setUp(self):
    self.workdir = tempfile.mkdtemp()

  #override
  def tearDown(self):
    shutil.rmtree(self.workdir)
    self.workdir = None

  def testUpload(self):
    version = '2.3.4'
    self.SetUpWorkdir(
        gms_lib=True,
        config_version=version,
        source_prop=True)

    status = update.main([
        'upload',
        '--dry-run',
        '--bucket', self.paths.bucket,
        '--config', self.paths.config_file,
        '--sdk-root', self.paths.gms.sdk_root
    ])
    self.assertEqual(status, 0, 'the command should have succeeded.')

    # bucket should contain license, name = content from LICENSE.sha1
    self.assertTrue(os.path.isfile(self.paths.config_license_sha1))
    license_sha1 = _GetFileContent(self.paths.config_license_sha1)
    bucket_license = os.path.join(self.paths.bucket, version, license_sha1)
    self.assertTrue(os.path.isfile(bucket_license))
    self.assertEqual(_GetFileContent(bucket_license), self.DEFAULT_LICENSE)

    # bucket should contain zip, name = content from zip.sha1
    self.assertTrue(os.path.isfile(self.paths.config_zip_sha1))
    bucket_zip = os.path.join(self.paths.bucket, str(version),
                              _GetFileContent(self.paths.config_zip_sha1))
    self.assertTrue(os.path.isfile(bucket_zip))

    # unzip, should contain expected files
    with zipfile.ZipFile(bucket_zip, "r") as bucket_zip_file:
      self.assertEqual(bucket_zip_file.namelist(),
                       ['com/google/android/gms/client/2.3.4/client-2.3.4.aar'])

  def testDownload(self):
    self.SetUpWorkdir(populate_bucket=True)

    with _MockedInput('y'):
      status = update.main([
          'download',
          '--dry-run',
          '--bucket', self.paths.bucket,
          '--config', self.paths.config_file,
          '--sdk-root', self.paths.gms.sdk_root,
      ])

    self.assertEqual(status, 0, 'the command should have succeeded.')

    # sdk_root should contain zip contents, zip sha1, license
    self.assertTrue(os.path.isfile(self.paths.gms.client_paths[0]))
    self.assertTrue(os.path.isfile(self.paths.gms.lib_zip_sha1))
    self.assertTrue(os.path.isfile(self.paths.gms.license))
    self.assertEquals(_GetFileContent(self.paths.gms.license),
                      self.DEFAULT_LICENSE)

  def testDownloadBot(self):
    self.SetUpWorkdir(populate_bucket=True, bot_env=True)

    # No need to type 'y' on bots
    status = update.main([
        'download',
        '--dry-run',
        '--bucket', self.paths.bucket,
        '--config', self.paths.config_file,
        '--sdk-root', self.paths.gms.sdk_root,
    ])

    self.assertEqual(status, 0, 'the command should have succeeded.')

    # sdk_root should contain zip contents, zip sha1, license
    self.assertTrue(os.path.isfile(self.paths.gms.client_paths[0]))
    self.assertTrue(os.path.isfile(self.paths.gms.lib_zip_sha1))
    self.assertTrue(os.path.isfile(self.paths.gms.license))
    self.assertEquals(_GetFileContent(self.paths.gms.license),
                      self.DEFAULT_LICENSE)

  def testDownloadAlreadyUpToDate(self):
    self.SetUpWorkdir(
        populate_bucket=True,
        existing_zip_sha1=self.DEFAULT_ZIP_SHA1)

    status = update.main([
        'download',
        '--dry-run',
        '--bucket', self.paths.bucket,
        '--config', self.paths.config_file,
        '--sdk-root', self.paths.gms.sdk_root,
    ])

    self.assertEqual(status, 0, 'the command should have succeeded.')

    # there should not be new files downloaded to sdk_root
    self.assertFalse(os.path.isfile(os.path.join(self.paths.gms.client_paths[0],
                                                 'dummy_file')))
    self.assertFalse(os.path.isfile(self.paths.gms.license))

  def testDownloadAcceptedLicense(self):
    self.SetUpWorkdir(
        populate_bucket=True,
        existing_license=self.DEFAULT_LICENSE)

    # License already accepted, no need to type
    status = update.main([
        'download',
        '--dry-run',
        '--bucket', self.paths.bucket,
        '--config', self.paths.config_file,
        '--sdk-root', self.paths.gms.sdk_root,
    ])

    self.assertEqual(status, 0, 'the command should have succeeded.')

    # sdk_root should contain zip contents, zip sha1, license
    self.assertTrue(os.path.isfile(self.paths.gms.client_paths[0]))
    self.assertTrue(os.path.isfile(self.paths.gms.lib_zip_sha1))
    self.assertTrue(os.path.isfile(self.paths.gms.license))
    self.assertEquals(_GetFileContent(self.paths.gms.license),
                      self.DEFAULT_LICENSE)

  def testDownloadNewLicense(self):
    self.SetUpWorkdir(
        populate_bucket=True,
        existing_license='Old license')

    with _MockedInput('y'):
      status = update.main([
          'download',
          '--dry-run',
          '--bucket', self.paths.bucket,
          '--config', self.paths.config_file,
          '--sdk-root', self.paths.gms.sdk_root,
      ])

    self.assertEqual(status, 0, 'the command should have succeeded.')

    # sdk_root should contain zip contents, zip sha1, NEW license
    self.assertTrue(os.path.isfile(self.paths.gms.client_paths[0]))
    self.assertTrue(os.path.isfile(self.paths.gms.lib_zip_sha1))
    self.assertTrue(os.path.isfile(self.paths.gms.license))
    self.assertEquals(_GetFileContent(self.paths.gms.license),
                      self.DEFAULT_LICENSE)

  def testDownloadRefusedLicense(self):
    self.SetUpWorkdir(
        populate_bucket=True,
        existing_license='Old license')

    with _MockedInput('n'):
      status = update.main([
          'download',
          '--dry-run',
          '--bucket', self.paths.bucket,
          '--config', self.paths.config_file,
          '--sdk-root', self.paths.gms.sdk_root,
      ])

    self.assertEqual(status, 0, 'the command should have succeeded.')

    # there should not be new files downloaded to sdk_root
    self.assertFalse(os.path.isfile(os.path.join(self.paths.gms.client_paths[0],
                                                 'dummy_file')))
    self.assertEquals(_GetFileContent(self.paths.gms.license),
                      'Old license')

  def testDownloadNoAndroidSDK(self):
    self.SetUpWorkdir(
        populate_bucket=True,
        existing_license='Old license')

    non_existing_sdk_root = os.path.join(self.workdir, 'non_existing_sdk_root')
    # Should not run, no typing needed
    status = update.main([
        'download',
        '--dry-run',
        '--bucket', self.paths.bucket,
        '--config', self.paths.config_file,
        '--sdk-root', non_existing_sdk_root,
    ])

    self.assertEqual(status, 0, 'the command should have succeeded.')
    self.assertFalse(os.path.isdir(non_existing_sdk_root))

  def SetUpWorkdir(self,
                   bot_env=False,
                   config_version=DEFAULT_CONFIG_VERSION,
                   existing_license=None,
                   existing_zip_sha1=None,
                   gms_lib=False,
                   populate_bucket=False,
                   source_prop=None):
    '''Prepares workdir by putting it in the specified state

    Args:
      - general
        bot_env: sets or unsets CHROME_HEADLESS

      - bucket
        populate_bucket: boolean. Populate the bucket with a zip and license
                         file. The sha1s will be copied to the config directory

      - config
        config_version: number. Version of the current SDK. Defaults to
                        `self.DEFAULT_CONFIG_VERSION`

      - sdk_root
        existing_license: string. Create a LICENSE file setting the specified
                          text as content of the currently accepted license.
        existing_zip_sha1: string. Create a sha1 file setting the specified
                           hash as hash of the SDK supposed to be installed
        gms_lib: boolean. Create a dummy file in the location of the play
                 services SDK.
        source_prop: boolean. Create a source.properties file that contains
                     the license to upload.
    '''
    client_name = 'client'
    self.paths = Paths(self.workdir, config_version, [client_name])

    # Create the main directories
    _MakeDirs(self.paths.gms.sdk_root)
    _MakeDirs(self.paths.config_dir)
    _MakeDirs(self.paths.bucket)

    # is not configured via argument.
    update.SHA1_DIRECTORY = self.paths.config_dir

    os.environ['CHROME_HEADLESS'] = '1' if bot_env else ''

    if config_version:
      _MakeDirs(os.path.dirname(self.paths.config_file))
      with open(self.paths.config_file, 'w') as stream:
        stream.write(('{"clients": ["%s"],'
                      '"version_number": "%s"}'
                      '\n') % (client_name, config_version))

    if existing_license:
      _MakeDirs(self.paths.gms.package)
      with open(self.paths.gms.license, 'w') as stream:
        stream.write(existing_license)

    if existing_zip_sha1:
      _MakeDirs(self.paths.gms.package)
      with open(self.paths.gms.lib_zip_sha1, 'w') as stream:
        stream.write(existing_zip_sha1)

    if gms_lib:
      _MakeDirs(os.path.dirname(self.paths.gms.client_paths[0]))
      with open(self.paths.gms.client_paths[0], 'w') as stream:
        stream.write('foo\n')

    if source_prop:
      _MakeDirs(os.path.dirname(self.paths.gms.source_prop))
      with open(self.paths.gms.source_prop, 'w') as stream:
        stream.write('Foo=Bar\n'
                     'Pkg.License=%s\n'
                     'Baz=Fizz\n' % self.DEFAULT_LICENSE)

    if populate_bucket:
      _MakeDirs(self.paths.config_dir)
      bucket_dir = os.path.join(self.paths.bucket, str(config_version))
      _MakeDirs(bucket_dir)

      # TODO(dgn) should we use real sha1s? comparison with the real sha1 is
      # done but does not do anything other than displaying a message.
      config_license_sha1 = 'license0and0filling0to0forty0chars000000'
      with open(self.paths.config_license_sha1, 'w') as stream:
        stream.write(config_license_sha1)

      with open(os.path.join(bucket_dir, config_license_sha1), 'w') as stream:
        stream.write(self.DEFAULT_LICENSE)

      config_zip_sha1 = self.DEFAULT_ZIP_SHA1
      with open(self.paths.config_zip_sha1, 'w') as stream:
        stream.write(config_zip_sha1)

      pre_zip_client = os.path.join(
          self.workdir,
          'pre_zip_lib',
          os.path.relpath(self.paths.gms.client_paths[0],
                          self.paths.gms.package))
      pre_zip_lib = os.path.dirname(pre_zip_client)
      post_zip_lib = os.path.join(bucket_dir, config_zip_sha1)
      print(pre_zip_lib, post_zip_lib)
      _MakeDirs(pre_zip_lib)
      with open(pre_zip_client, 'w') as stream:
        stream.write('foo\n')

      # pylint: disable=protected-access
      update._ZipLibrary(post_zip_lib, [pre_zip_client], os.path.join(
          self.workdir, 'pre_zip_lib'))

    if logging.getLogger().isEnabledFor(logging.DEBUG):
      cmd_helper.Call(['tree', self.workdir])


class Paths(object):
  '''Declaration of the paths commonly manipulated in the tests.'''

  def __init__(self, workdir, version, clients):
    self.bucket = os.path.join(workdir, 'bucket')

    self.config_dir = os.path.join(workdir, 'config')
    self.config_file = os.path.join(self.config_dir, 'config.json')
    self.config_license_sha1 = os.path.join(self.config_dir, 'LICENSE.sha1')
    self.config_zip_sha1 = os.path.join(
        self.config_dir,
        'google_play_services_library.zip.sha1')
    self.gms = update.PlayServicesPaths(os.path.join(workdir, 'sdk_root'),
                                        version, clients)


def _GetFileContent(file_path):
  with open(file_path, 'r') as stream:
    return stream.read()


def _MakeDirs(path):
  '''Avoids having to do the error handling everywhere.'''
  if not os.path.exists(path):
    os.makedirs(path)


@contextlib.contextmanager
def _MockedInput(typed_string):
  '''Makes raw_input return |typed_string| while inside the context.'''
  try:
    if isinstance(__builtins__, dict):
      original_raw_input = __builtins__['raw_input']
      __builtins__['raw_input'] = lambda _: typed_string
    else:
      original_raw_input = __builtins__.raw_input
      __builtins__.raw_input = lambda _: typed_string
    yield
  finally:
    if isinstance(__builtins__, dict):
      __builtins__['raw_input'] = original_raw_input
    else:
      __builtins__.raw_input = original_raw_input


if __name__ == '__main__':
  unittest.main()
