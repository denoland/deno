#!/usr/bin/env python
#
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Create an Android application bundle from one or more bundle modules."""

import argparse
import itertools
import json
import os
import shutil
import sys
import tempfile
import zipfile

# NOTE: Keep this consistent with the _create_app_bundle_py_imports definition
#       in build/config/android/rules.py
from util import build_utils
from util import resource_utils

import bundletool

# Location of language-based assets in bundle modules.
_LOCALES_SUBDIR = 'assets/locales/'

# The fallback language should always have its .pak files included in
# the base apk, i.e. not use language-based asset targetting. This ensures
# that Chrome won't crash on startup if its bundle is installed on a device
# with an unsupported system locale (e.g. fur-rIT).
_FALLBACK_LANGUAGE = 'en'

# List of split dimensions recognized by this tool.
_ALL_SPLIT_DIMENSIONS = [ 'ABI', 'SCREEN_DENSITY', 'LANGUAGE' ]

# Due to historical reasons, certain languages identified by Chromium with a
# 3-letters ISO 639-2 code, are mapped to a nearly equivalent 2-letters
# ISO 639-1 code instead (due to the fact that older Android releases only
# supported the latter when matching resources).
#
# the same conversion as for Java resources.
_SHORTEN_LANGUAGE_CODE_MAP = {
  'fil': 'tl',  # Filipino to Tagalog.
}

def _ParseArgs(args):
  parser = argparse.ArgumentParser()
  parser.add_argument('--out-bundle', required=True,
                      help='Output bundle zip archive.')
  parser.add_argument('--module-zips', required=True,
                      help='GN-list of module zip archives.')
  parser.add_argument('--uncompressed-assets', action='append',
                      help='GN-list of uncompressed assets.')
  parser.add_argument('--uncompress-shared-libraries', action='append',
                      help='Whether to store native libraries uncompressed. '
                      'This is a string to allow @FileArg usage.')
  parser.add_argument('--split-dimensions',
                      help="GN-list of split dimensions to support.")
  parser.add_argument('--keystore-path', help='Keystore path')
  parser.add_argument('--keystore-password', help='Keystore password')
  parser.add_argument('--key-name', help='Keystore key name')

  options = parser.parse_args(args)
  options.module_zips = build_utils.ParseGnList(options.module_zips)

  if len(options.module_zips) == 0:
    raise Exception('The module zip list cannot be empty.')

  # Signing is optional, but all --keyXX parameters should be set.
  if options.keystore_path or options.keystore_password or options.key_name:
    if not options.keystore_path or not options.keystore_password or \
        not options.key_name:
      raise Exception('When signing the bundle, use --keystore-path, '
                      '--keystore-password and --key-name.')

  # Merge all uncompressed assets into a set.
  uncompressed_list = []
  if options.uncompressed_assets:
    for l in options.uncompressed_assets:
      for entry in build_utils.ParseGnList(l):
        # Each entry has the following format: 'zipPath' or 'srcPath:zipPath'
        pos = entry.find(':')
        if pos >= 0:
          uncompressed_list.append(entry[pos + 1:])
        else:
          uncompressed_list.append(entry)

  options.uncompressed_assets = set(uncompressed_list)

  # Merge uncompressed native libs flags, they all must have the same value.
  if options.uncompress_shared_libraries:
    uncompressed_libs = set(options.uncompress_shared_libraries)
    if len(uncompressed_libs) > 1:
      parser.error('Inconsistent uses of --uncompress-native-libs!')
    options.uncompress_shared_libraries = 'True' in uncompressed_libs

  # Check that all split dimensions are valid
  if options.split_dimensions:
    options.split_dimensions = build_utils.ParseGnList(options.split_dimensions)
    for dim in options.split_dimensions:
      if dim.upper() not in _ALL_SPLIT_DIMENSIONS:
        parser.error('Invalid split dimension "%s" (expected one of: %s)' % (
            dim, ', '.join(x.lower() for x in _ALL_SPLIT_DIMENSIONS)))

  return options


def _MakeSplitDimension(value, enabled):
  """Return dict modelling a BundleConfig splitDimension entry."""
  return {'value': value, 'negate': not enabled}


def _GenerateBundleConfigJson(uncompressed_assets,
                              uncompress_shared_libraries,
                              split_dimensions):
  """Generate a dictionary that can be written to a JSON BuildConfig.

  Args:
    uncompressed_assets: A list or set of file paths under assets/ that always
      be stored uncompressed.
    uncompress_shared_libraries: Boolean, whether to uncompress all native libs.
    split_dimensions: list of split dimensions.
  Returns:
    A dictionary that can be written as a json file.
  """
  # Compute splitsConfig list. Each item is a dictionary that can have
  # the following keys:
  #    'value': One of ['LANGUAGE', 'DENSITY', 'ABI']
  #    'negate': Boolean, True to indicate that the bundle should *not* be
  #              split (unused at the moment by this script).

  split_dimensions = [ _MakeSplitDimension(dim, dim in split_dimensions)
                       for dim in _ALL_SPLIT_DIMENSIONS ]

  # Compute uncompressedGlob list.
  if uncompress_shared_libraries:
    uncompressed_globs = [
      'lib/*/*.so',        # All native libraries.
    ]
  else:
    uncompressed_globs = [
      'lib/*/crazy.*',     # Native libraries loaded by the crazy linker.
    ]

  uncompressed_globs.extend('assets/' + x for x in uncompressed_assets)

  data = {
    'optimizations': {
      'splitsConfig': {
        'splitDimension': split_dimensions,
      },
    },
    'compression': {
       'uncompressedGlob': sorted(uncompressed_globs),
    },
  }

  return json.dumps(data, indent=2)


def _RewriteLanguageAssetPath(src_path):
  """Rewrite the destination path of a locale asset for language-based splits.

  Should only be used when generating bundles with language-based splits.
  This will rewrite paths that look like locales/<locale>.pak into
  locales#<language>/<locale>.pak, where <language> is the language code
  from the locale.
  """
  if not src_path.startswith(_LOCALES_SUBDIR) or not src_path.endswith('.pak'):
    return src_path

  locale = src_path[len(_LOCALES_SUBDIR):-4]
  android_locale = resource_utils.CHROME_TO_ANDROID_LOCALE_MAP.get(
      locale, locale)

  # The locale format is <lang>-<region> or <lang>. Extract the language.
  pos = android_locale.find('-')
  if pos >= 0:
    android_language = android_locale[:pos]
  else:
    android_language = android_locale

  if android_language == _FALLBACK_LANGUAGE:
    return 'assets/locales/%s.pak' % locale

  return 'assets/locales#lang_%s/%s.pak' % (android_language, locale)


def _SplitModuleForAssetTargeting(src_module_zip, tmp_dir, split_dimensions):
  """Splits assets in a module if needed.

  Args:
    src_module_zip: input zip module path.
    tmp_dir: Path to temporary directory, where the new output module might
      be written to.
    split_dimensions: list of split dimensions.

  Returns:
    If the module doesn't need asset targeting, doesn't do anything and
    returns src_module_zip. Otherwise, create a new module zip archive under
    tmp_dir with the same file name, but which contains assets paths targeting
    the proper dimensions.
  """
  split_language = 'LANGUAGE' in split_dimensions
  if not split_language:
    # Nothing to target, so return original module path.
    return src_module_zip

  with zipfile.ZipFile(src_module_zip, 'r') as src_zip:
    language_files = [
      f for f in src_zip.namelist() if f.startswith(_LOCALES_SUBDIR)]

    if not language_files:
      # Not language-based assets to split in this module.
      return src_module_zip

    tmp_zip = os.path.join(tmp_dir, os.path.basename(src_module_zip))
    with zipfile.ZipFile(tmp_zip, 'w') as dst_zip:
      for info in src_zip.infolist():
        src_path = info.filename
        is_compressed = info.compress_type != zipfile.ZIP_STORED

        dst_path = src_path
        if src_path in language_files:
          dst_path = _RewriteLanguageAssetPath(src_path)

        build_utils.AddToZipHermetic(dst_zip, dst_path,
                                     data=src_zip.read(src_path),
                                     compress=is_compressed)

    return tmp_zip


def main(args):
  args = build_utils.ExpandFileArgs(args)
  options = _ParseArgs(args)

  split_dimensions = []
  if options.split_dimensions:
    split_dimensions = [x.upper() for x in options.split_dimensions]

  bundle_config = _GenerateBundleConfigJson(options.uncompressed_assets,
                                            options.uncompress_shared_libraries,
                                            split_dimensions)
  with build_utils.TempDir() as tmp_dir:
    module_zips = [
        _SplitModuleForAssetTargeting(module, tmp_dir, split_dimensions) \
        for module in options.module_zips]

    tmp_bundle = os.path.join(tmp_dir, 'tmp_bundle')

    tmp_unsigned_bundle = tmp_bundle
    if options.keystore_path:
      tmp_unsigned_bundle = tmp_bundle + '.unsigned'

    # Important: bundletool requires that the bundle config file is
    # named with a .pb.json extension.
    tmp_bundle_config = tmp_bundle + '.BundleConfig.pb.json'

    with open(tmp_bundle_config, 'w') as f:
      f.write(bundle_config)

    cmd_args = ['java', '-jar', bundletool.BUNDLETOOL_JAR_PATH, 'build-bundle']
    cmd_args += ['--modules=%s' % ','.join(module_zips)]
    cmd_args += ['--output=%s' % tmp_unsigned_bundle]
    cmd_args += ['--config=%s' % tmp_bundle_config]

    build_utils.CheckOutput(cmd_args, print_stdout=True, print_stderr=True)

    if options.keystore_path:
      # NOTE: As stated by the public documentation, apksigner cannot be used
      # to sign the bundle (because it rejects anything that isn't an APK).
      # The signature and digest algorithm selection come from the internal
      # App Bundle documentation. There is no corresponding public doc :-(
      signing_cmd_args = [
          'jarsigner', '-sigalg', 'SHA256withRSA', '-digestalg', 'SHA-256',
          '-keystore', 'file:' + options.keystore_path,
          '-storepass' , options.keystore_password,
          '-signedjar', tmp_bundle,
          tmp_unsigned_bundle,
          options.key_name,
      ]
      build_utils.CheckOutput(signing_cmd_args, print_stderr=True)

    shutil.move(tmp_bundle, options.out_bundle)


if __name__ == '__main__':
  main(sys.argv[1:])
