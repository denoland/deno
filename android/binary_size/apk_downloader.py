#!/usr/bin/env python
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import os
import sys
import zipfile

_BUILD_ANDROID = os.path.join(os.path.dirname(__file__), os.pardir)
sys.path.append(_BUILD_ANDROID)
from pylib.constants import host_paths

sys.path.append(os.path.join(_BUILD_ANDROID, 'gyp'))
from util import build_utils

sys.path.append(os.path.join(host_paths.DIR_SOURCE_ROOT, 'build'))
import find_depot_tools  # pylint: disable=import-error,unused-import
import download_from_google_storage
import upload_to_google_storage

CURRENT_MILESTONE = '67'
DEFAULT_BUCKET = 'gs://chromium-android-tools/apks'
DEFAULT_DOWNLOAD_PATH = os.path.join(os.path.dirname(__file__), 'apks')
DEFAULT_BUILDER = 'Android_Builder'
DEFAULT_APK = 'MonochromePublic.apk'
_ALL_BUILDER_APKS = {
  'Android Builder': ['ChromePublic.apk', 'ChromeModernPublic.apk',
                      'MonochromePublic.apk'],
  'Android arm64 Builder': ['ChromePublic.apk', 'ChromeModernPublic.apk'],
}


def MaybeDownloadApk(builder, milestone, apk, download_path, bucket):
  """Returns path to the downloaded APK or None if not found."""
  apk_path = os.path.join(download_path, builder, milestone, apk)
  sha1_path = apk_path + '.sha1'
  base_url = os.path.join(bucket, builder, milestone)
  if os.path.exists(apk_path):
    print '%s already exists' % apk_path
    return apk_path
  elif not os.path.exists(sha1_path):
    print 'Skipping %s, file not found' % sha1_path
    return None
  else:
    download_from_google_storage.download_from_google_storage(
        input_filename=sha1_path,
        sha1_file=sha1_path,
        base_url=base_url,
        gsutil=download_from_google_storage.Gsutil(
            download_from_google_storage.GSUTIL_DEFAULT_PATH),
        num_threads=1,
        directory=False,
        recursive=False,
        force=False,
        output=apk_path,
        ignore_errors=False,
        verbose=True,
        auto_platform=False,
        extract=False)
    return apk_path


def _UpdateReferenceApks(milestones):
  """Update reference APKs and creates .sha1 files ready for commit.

  Will fail if perf builders were broken for the given milestone (use next
  passing build in this case).
  """
  with build_utils.TempDir() as temp_dir:
    for milestone, crrev in milestones:
      for builder, apks in _ALL_BUILDER_APKS.iteritems():
        tools_builder_path = builder.replace(' ', '_')
        zip_path = os.path.join(temp_dir, 'build_product.zip')
        commit = build_utils.CheckOutput(['git', 'crrev-parse', crrev]).strip()
        # Download build product from perf builders.
        build_utils.CheckOutput([
            'gsutil', 'cp', 'gs://chrome-perf/%s/full-build-linux_%s.zip' % (
            builder, commit), zip_path])

        # Extract desired .apks.
        with zipfile.ZipFile(zip_path) as z:
          in_zip_paths = z.namelist()
          out_dir = os.path.commonprefix(in_zip_paths)
          for apk_name in apks:
            output_path = os.path.join(
                DEFAULT_DOWNLOAD_PATH, tools_builder_path, milestone)
            apk_path = os.path.join(out_dir, 'apks', apk_name)
            zip_info = z.getinfo(apk_path)
            zip_info.filename = apk_path.replace(apk_path, apk_name)
            z.extract(zip_info, output_path)
            input_files = [os.path.join(output_path, apk_name)]
            bucket_path = os.path.join(
                DEFAULT_BUCKET, tools_builder_path, milestone)

            # Upload .apks to chromium-android-tools so that they aren't
            # automatically removed in the future.
            upload_to_google_storage.upload_to_google_storage(
                input_files,
                bucket_path,
                upload_to_google_storage.Gsutil(
                    upload_to_google_storage.GSUTIL_DEFAULT_PATH),
                False,  # force
                False,  # use_md5
                10,  # num_threads
                False,  # skip_hashing
                None)  # gzip


def main():
  argparser = argparse.ArgumentParser(
      description='Utility for downloading archived APKs used for measuring '
                  'per-milestone patch size growth.',
      formatter_class=argparse.ArgumentDefaultsHelpFormatter)
  argparser.add_argument('--download-path', default=DEFAULT_DOWNLOAD_PATH,
                         help='Directory to store downloaded APKs.')
  argparser.add_argument('--milestone', default=CURRENT_MILESTONE,
                         help='Download reference APK for this milestone.')
  argparser.add_argument('--apk', default=DEFAULT_APK, help='APK name.')
  argparser.add_argument('--builder', default=DEFAULT_BUILDER,
                         help='Builder name.')
  argparser.add_argument('--bucket', default=DEFAULT_BUCKET,
                         help='Google storage bucket where APK is stored.')
  argparser.add_argument('--update', action='append', nargs=2,
                        help='List of MILESTONE CRREV pairs to upload '
                        'reference APKs for. Mutally exclusive with '
                        'downloading reference APKs.')
  args = argparser.parse_args()
  if args.update:
    _UpdateReferenceApks(args.update)
  else:
    MaybeDownloadApk(args.builder, args.milestone, args.apk,
                     args.download_path, args.bucket)


if __name__ == '__main__':
  sys.exit(main())
