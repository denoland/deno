#!/usr/bin/env python
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

'''
Script to help uploading and downloading the Google Play services library to
and from a Google Cloud storage.
'''

import argparse
import logging
import os
import re
import shutil
import sys
import zipfile

sys.path.append(os.path.join(os.path.dirname(__file__), os.pardir))
import devil_chromium
from devil.utils import cmd_helper
from play_services import utils
from py_utils import tempfile_ext
from pylib import constants
from pylib.constants import host_paths
from pylib.utils import logging_utils
from pylib.utils import maven_downloader

sys.path.append(os.path.join(host_paths.DIR_SOURCE_ROOT, 'build'))
import find_depot_tools  # pylint: disable=import-error,unused-import
import breakpad
import download_from_google_storage
import upload_to_google_storage


# Directory where the SHA1 files for the zip and the license are stored
# It should be managed by git to provided information about new versions.
SHA1_DIRECTORY = os.path.join(host_paths.DIR_SOURCE_ROOT, 'build', 'android',
                              'play_services')

# Default bucket used for storing the files.
GMS_CLOUD_STORAGE = 'chromium-android-tools/play-services'

# Path to the default configuration file. It exposes the currently installed
# version of the library in a human readable way.
CONFIG_DEFAULT_PATH = os.path.join(host_paths.DIR_SOURCE_ROOT, 'build',
                                   'android', 'play_services', 'config.json')

LICENSE_FILE_NAME = 'LICENSE'
ZIP_FILE_NAME = 'google_play_services_library.zip'

LICENSE_PATTERN = re.compile(r'^Pkg\.License=(?P<text>.*)$', re.MULTILINE)

COM_GOOGLE_ANDROID_GMS = os.path.join('com', 'google', 'android', 'gms')
EXTRAS_GOOGLE_M2REPOSITORY = os.path.join('extras', 'google', 'm2repository')

def main(raw_args):
  parser = argparse.ArgumentParser(
      description=__doc__ + 'Please see the subcommand help for more details.',
      formatter_class=utils.DefaultsRawHelpFormatter)
  subparsers = parser.add_subparsers(title='commands')

  # Download arguments
  parser_download = subparsers.add_parser(
      'download',
      help='download the library from the cloud storage',
      description=Download.__doc__,
      formatter_class=utils.DefaultsRawHelpFormatter)
  parser_download.set_defaults(func=Download)
  AddBasicArguments(parser_download)
  AddBucketArguments(parser_download)

  # SDK Update arguments
  parser_sdk = subparsers.add_parser(
      'sdk',
      help='get the latest Google Play services SDK using Maven',
      description=UpdateSdk.__doc__,
      formatter_class=utils.DefaultsRawHelpFormatter)
  parser_sdk.set_defaults(func=UpdateSdk)
  AddBasicArguments(parser_sdk)

  # Upload arguments
  parser_upload = subparsers.add_parser(
      'upload',
      help='upload the library to the cloud storage',
      description=Upload.__doc__,
      formatter_class=utils.DefaultsRawHelpFormatter)

  parser_upload.set_defaults(func=Upload)
  AddBasicArguments(parser_upload)
  AddBucketArguments(parser_upload)

  args = parser.parse_args(raw_args)
  if args.verbose:
    logging.basicConfig(level=logging.DEBUG)
  logging_utils.ColorStreamHandler.MakeDefault(not _IsBotEnvironment())
  devil_chromium.Initialize()
  return args.func(args)


def AddBasicArguments(parser):
  '''
  Defines the common arguments on subparser rather than the main one. This
  allows to put arguments after the command: `foo.py upload --debug --force`
  instead of `foo.py --debug upload --force`
  '''

  parser.add_argument('--config',
                      help='JSON Configuration file',
                      default=CONFIG_DEFAULT_PATH)

  parser.add_argument('--sdk-root',
                      help='base path to the Android SDK tools root',
                      default=constants.ANDROID_SDK_ROOT)

  parser.add_argument('-v', '--verbose',
                      action='store_true',
                      help='print debug information')


def AddBucketArguments(parser):
  parser.add_argument('--bucket',
                      help='name of the bucket where the files are stored',
                      default=GMS_CLOUD_STORAGE)

  parser.add_argument('--dry-run',
                      action='store_true',
                      help=('run the script in dry run mode. Files will be '
                            'copied to a local directory instead of the '
                            'cloud storage. The bucket name will be as path '
                            'to that directory relative to the repository '
                            'root.'))

  parser.add_argument('-f', '--force',
                      action='store_true',
                      help='run even if the library is already up to date')


def Download(args):
  '''
  Downloads the Google Play services library from a Google Cloud Storage bucket
  and installs it to
  //third_party/android_tools/sdk/extras/google/m2repository.

  A license check will be made, and the user might have to accept the license
  if that has not been done before.
  '''

  if not os.path.isdir(args.sdk_root):
    logging.debug('Did not find the Android SDK root directory at "%s".',
                  args.sdk_root)
    if not args.force:
      logging.info('Skipping, not on an android checkout.')
      return 0

  config = utils.ConfigParser(args.config)
  paths = PlayServicesPaths(args.sdk_root, config.version_number,
                            config.clients)

  if os.path.isdir(paths.package) and not os.access(paths.package, os.W_OK):
    logging.error('Failed updating the Google Play Services library. '
                  'The location is not writable. Please remove the '
                  'directory (%s) and try again.', paths.package)
    return -2

  new_lib_zip_sha1 = os.path.join(SHA1_DIRECTORY, ZIP_FILE_NAME + '.sha1')

  logging.debug('Comparing zip hashes: %s and %s', new_lib_zip_sha1,
                paths.lib_zip_sha1)
  if utils.FileEquals(new_lib_zip_sha1, paths.lib_zip_sha1) and not args.force:
    logging.info('Skipping, the Google Play services library is up to date.')
    return 0

  bucket_path = _VerifyBucketPathFormat(args.bucket,
                                        config.version_number,
                                        args.dry_run)

  with tempfile_ext.NamedTemporaryDirectory() as tmp_root:
    # setup the destination directory
    _MakeDirIfAbsent(paths.package)

    # download license file from bucket/{version_number}/license.sha1
    new_license = os.path.join(tmp_root, LICENSE_FILE_NAME)

    license_sha1 = os.path.join(SHA1_DIRECTORY, LICENSE_FILE_NAME + '.sha1')
    _DownloadFromBucket(bucket_path, license_sha1, new_license,
                        args.verbose, args.dry_run)

    if (not _IsBotEnvironment() and
        not _CheckLicenseAgreement(new_license, paths.license,
                                   config.version_number)):
        logging.warning('Your version of the Google Play services library is '
                        'not up to date. You might run into issues building '
                        'or running the app. Please run `%s download` to '
                        'retry downloading it.', __file__)
        return 0

    new_lib_zip = os.path.join(tmp_root, ZIP_FILE_NAME)
    _DownloadFromBucket(bucket_path, new_lib_zip_sha1, new_lib_zip,
                        args.verbose, args.dry_run)

    try:
      # Remove the deprecated sdk directory.
      deprecated_package_path = os.path.join(args.sdk_root, 'extras', 'google',
                                             'google_play_services')
      if os.path.exists(deprecated_package_path):
        shutil.rmtree(deprecated_package_path)

      # We remove the current version of the Google Play services SDK.
      if os.path.exists(paths.package):
        shutil.rmtree(paths.package)
      os.makedirs(paths.package)

      logging.debug('Extracting the library to %s', paths.package)
      with zipfile.ZipFile(new_lib_zip, "r") as new_lib_zip_file:
        new_lib_zip_file.extractall(paths.package)

      logging.debug('Copying %s to %s', new_license, paths.license)
      shutil.copy(new_license, paths.license)

      logging.debug('Copying %s to %s', new_lib_zip_sha1, paths.lib_zip_sha1)
      shutil.copy(new_lib_zip_sha1, paths.lib_zip_sha1)

      logging.info('Update complete.')

    except Exception as e:  # pylint: disable=broad-except
      logging.error('Failed updating the Google Play Services library. '
                    'An error occurred while installing the new version in '
                    'the SDK directory: %s ', e)
      return -3

  return 0


def _MakeDirIfAbsent(path):
  try:
    os.makedirs(path)
  except OSError as e:
    if e.errno != os.errno.EEXIST:
      raise


def UpdateSdk(args):
  '''
  Uses Maven to download the latest Google Play Services SDK. Its installation
  path is //third_party/android_tools/sdk/extras/google/m2repository.
  '''

  # This should function should not run on bots and could fail for many user
  # and setup related reasons. Also, exceptions here are not caught, so we
  # disable breakpad to avoid spamming the logs.
  breakpad.IS_ENABLED = False

  config = utils.ConfigParser(args.config)
  target_repo = os.path.join(args.sdk_root, EXTRAS_GOOGLE_M2REPOSITORY)

  # Remove the old SDK.
  # TODO(https://crbug.com/778650) not everything should be deleted.
  shutil.rmtree(target_repo, ignore_errors=True)

  downloader = maven_downloader.MavenDownloader()
  artifact_list = [
      'com.google.android.gms:{}:{}:aar'.format(client, config.version_number)
      for client in config.clients]
  downloader.Install(target_repo, artifact_list)
  return 0


def Upload(args):
  '''
  Uploads the library from the local Google Play services SDK to a Google Cloud
  storage bucket. The version of the library and the list of clients to be
  uploaded will be taken from the configuration file. (see --config parameter)

  By default, a local commit will be made at the end of the operation.
  '''

  # This should function should not run on bots and could fail for many user
  # and setup related reasons. Also, exceptions here are not caught, so we
  # disable breakpad to avoid spamming the logs.
  breakpad.IS_ENABLED = False

  config = utils.ConfigParser(args.config)
  paths = PlayServicesPaths(args.sdk_root, config.version_number,
                            config.clients)
  logging.debug('-- Loaded paths --\n%s\n------------------', paths)

  with tempfile_ext.NamedTemporaryDirectory() as tmp_root:
    new_lib_zip = os.path.join(tmp_root, ZIP_FILE_NAME)
    new_license = os.path.join(tmp_root, LICENSE_FILE_NAME)

    _ZipLibrary(new_lib_zip, paths.client_paths, paths.package)
    _ExtractLicenseFile(new_license, paths.source_prop)

    bucket_path = _VerifyBucketPathFormat(args.bucket, config.version_number,
                                          args.dry_run)
    files_to_upload = [new_lib_zip, new_license]
    logging.debug('Uploading %s to %s', files_to_upload, bucket_path)
    _UploadToBucket(bucket_path, files_to_upload, args.dry_run)

    new_lib_zip_sha1 = os.path.join(SHA1_DIRECTORY,
                                    ZIP_FILE_NAME + '.sha1')
    new_license_sha1 = os.path.join(SHA1_DIRECTORY,
                                    LICENSE_FILE_NAME + '.sha1')
    shutil.copy(new_lib_zip + '.sha1', new_lib_zip_sha1)
    shutil.copy(new_license + '.sha1', new_license_sha1)

  logging.info('Update to version %s complete', config.version_number)
  return 0


def _DownloadFromBucket(bucket_path, sha1_file, destination, verbose,
                        is_dry_run):
  '''Downloads the file designated by the provided sha1 from a cloud bucket.'''

  download_from_google_storage.download_from_google_storage(
      input_filename=sha1_file,
      base_url=bucket_path,
      gsutil=_InitGsutil(is_dry_run),
      num_threads=1,
      directory=None,
      recursive=False,
      force=False,
      output=destination,
      ignore_errors=False,
      sha1_file=sha1_file,
      verbose=verbose,
      auto_platform=True,
      extract=False)


def _UploadToBucket(bucket_path, files_to_upload, is_dry_run):
  '''Uploads the files designated by the provided paths to a cloud bucket. '''

  upload_to_google_storage.upload_to_google_storage(
      input_filenames=files_to_upload,
      base_url=bucket_path,
      gsutil=_InitGsutil(is_dry_run),
      force=False,
      use_md5=False,
      num_threads=1,
      skip_hashing=False,
      gzip=None)


def _InitGsutil(is_dry_run):
  '''Initialize the Gsutil object as regular or dummy version for dry runs. '''

  if is_dry_run:
    return DummyGsutil()
  else:
    return download_from_google_storage.Gsutil(
        download_from_google_storage.GSUTIL_DEFAULT_PATH)


def _ExtractLicenseFile(license_path, prop_file_path):
  with open(prop_file_path, 'r') as prop_file:
    prop_file_content = prop_file.read()

  match = LICENSE_PATTERN.search(prop_file_content)
  if not match:
    raise AttributeError('The license was not found in ' +
                         os.path.abspath(prop_file_path))

  with open(license_path, 'w') as license_file:
    license_file.write(match.group('text'))


def _CheckLicenseAgreement(expected_license_path, actual_license_path,
                           version_number):
  '''
  Checks that the new license is the one already accepted by the user. If it
  isn't, it prompts the user to accept it. Returns whether the expected license
  has been accepted.
  '''

  if utils.FileEquals(expected_license_path, actual_license_path):
    return True

  with open(expected_license_path) as license_file:
    # Uses plain print rather than logging to make sure this is not formatted
    # by the logger.
    print ('Updating the Google Play services SDK to '
           'version %s.' % version_number)

    # The output is buffered when running as part of gclient hooks. We split
    # the text here and flush is explicitly to avoid having part of it dropped
    # out.
    # Note: text contains *escaped* new lines, so we split by '\\n', not '\n'.
    for license_part in license_file.read().split('\\n'):
      print license_part
      sys.stdout.flush()

  # Need to put the prompt on a separate line otherwise the gclient hook buffer
  # only prints it after we received an input.
  print ('Do you accept the license for version %s of the Google Play services '
         'client library? [y/n]: ' % version_number)
  sys.stdout.flush()
  return raw_input('> ') in ('Y', 'y')


def _IsBotEnvironment():
  return bool(os.environ.get('CHROME_HEADLESS'))


def _VerifyBucketPathFormat(bucket_name, version_number, is_dry_run):
  '''
  Formats and checks the download/upload path depending on whether we are
  running in dry run mode or not. Returns a supposedly safe path to use with
  Gsutil.
  '''

  if is_dry_run:
    bucket_path = os.path.abspath(os.path.join(bucket_name,
                                               str(version_number)))
    if not os.path.isdir(bucket_path):
      os.makedirs(bucket_path)
  else:
    if bucket_name.startswith('gs://'):
      # We enforce the syntax without gs:// for consistency with the standalone
      # download/upload scripts and to make dry run transition easier.
      raise AttributeError('Please provide the bucket name without the gs:// '
                           'prefix (e.g. %s)' % GMS_CLOUD_STORAGE)
    bucket_path = 'gs://%s/%s' % (bucket_name, version_number)

  return bucket_path

def _ZipLibrary(zip_name, files, zip_root):
  with zipfile.ZipFile(zip_name, 'w', zipfile.ZIP_DEFLATED) as zipf:
    for file_name in files:
      zipf.write(file_name, os.path.relpath(file_name, zip_root))


class PlayServicesPaths(object):
  '''
  Describes the different paths to be used in the update process.

         Filesystem hierarchy                        | Exposed property / notes
  ---------------------------------------------------|-------------------------
  [sdk_root]                                         | sdk_root / (1)
   +- extras                                         |
      +- google                                      |
         +- m2repository                             | package / (2)
            +- source.properties                     | source_prop / (3)
            +- LICENSE                               | license / (4)
            +- google_play_services_library.zip.sha1 | lib_zip_sha1 / (5)
            +- com/google/android/gms/               |
               +- [play-services-foo]                |
                  +- [X.Y.Z]                         |
                     +- play-services-foo-X.Y.Z.aar  | client_paths / (6)

  Notes:

   1. sdk_root: Path provided as a parameter to the script (--sdk_root)
   2. package: This directory contains the Google Play services SDK itself.
      When downloaded via the Android SDK manager, it will be a complete maven,
      repository with the different versions of the library. When the update
      script downloads the library from our cloud storage, it is cleared.
   3. source_prop: File created by the Android SDK manager that contains
      the package information, such as the version info and the license.
   4. license: File created by the update script. Contains the license accepted
      by the user.
   5. lib_zip_sha1: sha1 of the library that has been installed by the
      update script. It is compared with the one required by the config file to
      check if an update is necessary.
   6. client_paths: The client library jars we care about. They are zipped
      zipped together and uploaded to the cloud storage.

  '''

  def __init__(self, sdk_root, version_number, client_names):
    '''
    sdk_root: path to the root of the sdk directory
    version_number: version of the library supposed to be installed locally.
    client_names: names of client libraries to be uploaded. See
        utils.ConfigParser for more info.
    '''
    self.sdk_root = sdk_root
    self.version_number = version_number

    self.package = os.path.join(sdk_root, EXTRAS_GOOGLE_M2REPOSITORY)
    self.lib_zip_sha1 = os.path.join(self.package, ZIP_FILE_NAME + '.sha1')
    self.license = os.path.join(self.package, LICENSE_FILE_NAME)
    self.source_prop = os.path.join(self.package, 'source.properties')

    self.client_paths = []
    for client in client_names:
      self.client_paths.append(os.path.join(
          self.package, COM_GOOGLE_ANDROID_GMS, client, version_number,
          '{}-{}.aar'.format(client, version_number)))

  def __repr__(self):
    return ("\nsdk_root: " + self.sdk_root +
            "\nversion_number: " + self.version_number +
            "\npackage: " + self.package +
            "\nlib_zip_sha1: " + self.lib_zip_sha1 +
            "\nlicense: " + self.license +
            "\nsource_prop: " + self.source_prop +
            "\nclient_paths: \n - " + '\n - '.join(self.client_paths))


class DummyGsutil(download_from_google_storage.Gsutil):
  '''
  Class that replaces Gsutil to use a local directory instead of an online
  bucket. It relies on the fact that Gsutil commands are very similar to shell
  ones, so for the ones used here (ls, cp), it works to just use them with a
  local directory.
  '''

  def __init__(self):
    super(DummyGsutil, self).__init__(
        download_from_google_storage.GSUTIL_DEFAULT_PATH)

  def call(self, *args):
    logging.debug('Calling command "%s"', str(args))
    return cmd_helper.GetCmdStatusOutputAndError(args)

  def check_call(self, *args):
    logging.debug('Calling command "%s"', str(args))
    return cmd_helper.GetCmdStatusOutputAndError(args)


if __name__ == '__main__':
  sys.exit(main(sys.argv[1:]))
