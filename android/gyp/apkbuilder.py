#!/usr/bin/env python
#
# Copyright (c) 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Adds the code parts to a resource APK."""

import argparse
import itertools
import os
import shutil
import sys
import tempfile
import zipfile

import finalize_apk

from util import build_utils


# Taken from aapt's Package.cpp:
_NO_COMPRESS_EXTENSIONS = ('.jpg', '.jpeg', '.png', '.gif', '.wav', '.mp2',
                           '.mp3', '.ogg', '.aac', '.mpg', '.mpeg', '.mid',
                           '.midi', '.smf', '.jet', '.rtttl', '.imy', '.xmf',
                           '.mp4', '.m4a', '.m4v', '.3gp', '.3gpp', '.3g2',
                           '.3gpp2', '.amr', '.awb', '.wma', '.wmv', '.webm')


def _ParseArgs(args):
  parser = argparse.ArgumentParser()
  build_utils.AddDepfileOption(parser)
  parser.add_argument('--assets',
                      help='GYP-list of files to add as assets in the form '
                           '"srcPath:zipPath", where ":zipPath" is optional.',
                      default='[]')
  parser.add_argument('--java-resources',
                      help='GYP-list of java_resources JARs to include.',
                      default='[]')
  parser.add_argument('--write-asset-list',
                      action='store_true',
                      help='Whether to create an assets/assets_list file.')
  parser.add_argument('--uncompressed-assets',
                      help='Same as --assets, except disables compression.',
                      default='[]')
  parser.add_argument('--resource-apk',
                      help='An .ap_ file built using aapt',
                      required=True)
  parser.add_argument('--output-apk',
                      help='Path to the output file',
                      required=True)
  parser.add_argument('--format', choices=['apk', 'bundle-module'],
                      default='apk', help='Specify output format.')
  parser.add_argument('--apk-pak-info-path',
                      help='Path to the *.apk.pak.info file')
  parser.add_argument('--apk-res-info-path',
                      help='Path to the *.apk.res.info file')
  parser.add_argument('--dex-file',
                      help='Path to the classes.dex to use')
  parser.add_argument('--native-libs',
                      action='append',
                      help='GYP-list of native libraries to include. '
                           'Can be specified multiple times.',
                      default=[])
  parser.add_argument('--secondary-native-libs',
                      action='append',
                      help='GYP-list of native libraries for secondary '
                           'android-abi. Can be specified multiple times.',
                      default=[])
  parser.add_argument('--android-abi',
                      help='Android architecture to use for native libraries')
  parser.add_argument('--secondary-android-abi',
                      help='The secondary Android architecture to use for'
                           'secondary native libraries')
  parser.add_argument('--native-lib-placeholders',
                      help='GYP-list of native library placeholders to add.',
                      default='[]')
  parser.add_argument('--secondary-native-lib-placeholders',
                      help='GYP-list of native library placeholders to add '
                           'for the secondary ABI',
                      default='[]')
  parser.add_argument('--uncompress-shared-libraries', default='False',
      choices=['true', 'True', 'false', 'False'],
      help='Whether to uncompress native shared libraries. Argument must be '
           'a boolean value.')
  parser.add_argument('--apksigner-path',
                      help='Path to the apksigner executable.')
  parser.add_argument('--zipalign-path',
                      help='Path to the zipalign executable.')
  parser.add_argument('--key-path',
                      help='Path to keystore for signing.')
  parser.add_argument('--key-passwd',
                      help='Keystore password')
  parser.add_argument('--key-name',
                      help='Keystore name')
  options = parser.parse_args(args)
  options.assets = build_utils.ParseGnList(options.assets)
  options.uncompressed_assets = build_utils.ParseGnList(
      options.uncompressed_assets)
  options.native_lib_placeholders = build_utils.ParseGnList(
      options.native_lib_placeholders)
  options.secondary_native_lib_placeholders = build_utils.ParseGnList(
      options.secondary_native_lib_placeholders)
  options.java_resources = build_utils.ParseGnList(options.java_resources)
  all_libs = []
  for gyp_list in options.native_libs:
    all_libs.extend(build_utils.ParseGnList(gyp_list))
  options.native_libs = all_libs
  secondary_libs = []
  for gyp_list in options.secondary_native_libs:
    secondary_libs.extend(build_utils.ParseGnList(gyp_list))
  options.secondary_native_libs = secondary_libs

  # --apksigner-path, --zipalign-path, --key-xxx arguments are
  # required when building an APK, but not a bundle module.
  if options.format == 'apk':
    required_args = ['apksigner_path', 'zipalign_path', 'key_path',
                     'key_passwd', 'key_name']
    for required in required_args:
      if not vars(options)[required]:
        raise Exception('Argument --%s is required for APKs.' % (
            required.replace('_', '-')))

  options.uncompress_shared_libraries = \
      options.uncompress_shared_libraries in [ 'true', 'True' ]

  if not options.android_abi and (options.native_libs or
                                  options.native_lib_placeholders):
    raise Exception('Must specify --android-abi with --native-libs')
  if not options.secondary_android_abi and (options.secondary_native_libs or
      options.secondary_native_lib_placeholders):
    raise Exception('Must specify --secondary-android-abi with'
                    ' --secondary-native-libs')
  return options


def _SplitAssetPath(path):
  """Returns (src, dest) given an asset path in the form src[:dest]."""
  path_parts = path.split(':')
  src_path = path_parts[0]
  if len(path_parts) > 1:
    dest_path = path_parts[1]
  else:
    dest_path = os.path.basename(src_path)
  return src_path, dest_path


def _ExpandPaths(paths):
  """Converts src:dst into tuples and enumerates files within directories.

  Args:
    paths: Paths in the form "src_path:dest_path"

  Returns:
    A list of (src_path, dest_path) tuples sorted by dest_path (for stable
    ordering within output .apk).
  """
  ret = []
  for path in paths:
    src_path, dest_path = _SplitAssetPath(path)
    if os.path.isdir(src_path):
      for f in build_utils.FindInDirectory(src_path, '*'):
        ret.append((f, os.path.join(dest_path, f[len(src_path) + 1:])))
    else:
      ret.append((src_path, dest_path))
  ret.sort(key=lambda t:t[1])
  return ret


def _AddAssets(apk, path_tuples, disable_compression=False):
  """Adds the given paths to the apk.

  Args:
    apk: ZipFile to write to.
    paths: List of paths (with optional :zipPath suffix) to add.
    disable_compression: Whether to disable compression.
  """
  # Group all uncompressed assets together in the hope that it will increase
  # locality of mmap'ed files.
  for target_compress in (False, True):
    for src_path, dest_path in path_tuples:

      compress = not disable_compression and (
          os.path.splitext(src_path)[1] not in _NO_COMPRESS_EXTENSIONS)
      if target_compress == compress:
        apk_path = 'assets/' + dest_path
        try:
          apk.getinfo(apk_path)
          # Should never happen since write_build_config.py handles merging.
          raise Exception('Multiple targets specified the asset path: %s' %
                          apk_path)
        except KeyError:
          build_utils.AddToZipHermetic(apk, apk_path, src_path=src_path,
                                       compress=compress)


def _CreateAssetsList(path_tuples):
  """Returns a newline-separated list of asset paths for the given paths."""
  dests = sorted(t[1] for t in path_tuples)
  return '\n'.join(dests) + '\n'


def _AddNativeLibraries(out_apk, native_libs, android_abi, uncompress):
  """Add native libraries to APK."""
  has_crazy_linker = any('android_linker' in os.path.basename(p)
                         for p in native_libs)
  for path in native_libs:
    basename = os.path.basename(path)

    compress = None
    if (uncompress and os.path.splitext(basename)[1] == '.so'
        and 'android_linker' not in basename
        and 'clang_rt' not in basename):
      compress = False
      # Add prefix to prevent android install from extracting upon install.
      if has_crazy_linker:
        basename = 'crazy.' + basename

    apk_path = 'lib/%s/%s' % (android_abi, basename)
    build_utils.AddToZipHermetic(out_apk,
                                 apk_path,
                                 src_path=path,
                                 compress=compress)


def _MergeResInfoFiles(res_info_path, resource_apk):
  resource_apk_info_path = resource_apk + '.info'
  shutil.copy(resource_apk_info_path, res_info_path)


def _FilterPakInfoPaths(assets):
  return [f.split(':')[0] + '.info' for f in assets if f.endswith('.pak')]


def _MergePakInfoFiles(merged_path, pak_infos):
  info_lines = set()
  for pak_info_path in pak_infos:
    with open(pak_info_path, 'r') as src_info_file:
      info_lines.update(src_info_file.readlines())
  with open(merged_path, 'w') as merged_info_file:
    merged_info_file.writelines(sorted(info_lines))


def main(args):
  args = build_utils.ExpandFileArgs(args)
  options = _ParseArgs(args)

  native_libs = sorted(options.native_libs)

  input_paths = [options.resource_apk, __file__]
  # Include native libs in the depfile_deps since GN doesn't know about the
  # dependencies when is_component_build=true.
  depfile_deps = list(native_libs)

  secondary_native_libs = []
  if options.secondary_native_libs:
    secondary_native_libs = sorted(options.secondary_native_libs)
    depfile_deps += secondary_native_libs

  if options.dex_file:
    input_paths.append(options.dex_file)

  input_strings = [options.android_abi,
                   options.native_lib_placeholders,
                   options.secondary_native_lib_placeholders,
                   str(options.uncompress_shared_libraries)]

  if options.secondary_android_abi:
    input_strings.append(options.secondary_android_abi)

  if options.java_resources:
    # Included via .build_config, so need to write it to depfile.
    depfile_deps.extend(options.java_resources)

  assets = _ExpandPaths(options.assets)
  uncompressed_assets = _ExpandPaths(options.uncompressed_assets)

  if options.apk_pak_info_path:
    pak_infos = _FilterPakInfoPaths(
        options.assets + options.uncompressed_assets)
    depfile_deps.extend(pak_infos)

  for src_path, dest_path in itertools.chain(assets, uncompressed_assets):
    # Included via .build_config, so need to write it to depfile.
    depfile_deps.append(src_path)
    input_strings.append(dest_path)

  output_paths = [options.output_apk]
  if options.apk_pak_info_path:
    output_paths.append(options.apk_pak_info_path)
  if options.apk_res_info_path:
    output_paths.append(options.apk_res_info_path)

  # Bundle modules have a structure similar to APKs, except that resources
  # are compiled in protobuf format (instead of binary xml), and that some
  # files are located into different top-level directories, e.g.:
  #  AndroidManifest.xml -> manifest/AndroidManifest.xml
  #  classes.dex -> dex/classes.dex
  #  res/ -> res/  (unchanged)
  #  assets/ -> assets/  (unchanged)
  #  <other-file> -> root/<other-file>
  #
  # Hence, the following variables are used to control the location of files in
  # the final archive.
  if options.format == 'bundle-module':
    apk_manifest_dir = 'manifest/'
    apk_root_dir = 'root/'
    apk_dex_dir = 'dex/'
  else:
    apk_manifest_dir = ''
    apk_root_dir = ''
    apk_dex_dir = ''

  def on_stale_md5():
    with tempfile.NamedTemporaryFile() as tmp_apk:
      tmp_file = tmp_apk.name
      with zipfile.ZipFile(options.resource_apk) as resource_apk, \
           zipfile.ZipFile(tmp_file, 'w', zipfile.ZIP_DEFLATED) as out_apk:
        def copy_resource(zipinfo, out_dir=''):
          compress = zipinfo.compress_type != zipfile.ZIP_STORED
          build_utils.AddToZipHermetic(out_apk, out_dir + zipinfo.filename,
                                       data=resource_apk.read(zipinfo.filename),
                                       compress=compress)

        # Make assets come before resources in order to maintain the same file
        # ordering as GYP / aapt. http://crbug.com/561862
        resource_infos = resource_apk.infolist()

        # 1. AndroidManifest.xml
        assert resource_infos[0].filename == 'AndroidManifest.xml'
        copy_resource(resource_infos[0], out_dir=apk_manifest_dir)

        # 2. Assets
        if options.write_asset_list:
          data = _CreateAssetsList(
              itertools.chain(assets, uncompressed_assets))
          build_utils.AddToZipHermetic(out_apk, 'assets/assets_list', data=data)

        _AddAssets(out_apk, assets, disable_compression=False)
        _AddAssets(out_apk, uncompressed_assets, disable_compression=True)

        # 3. Dex files
        if options.dex_file and options.dex_file.endswith('.zip'):
          with zipfile.ZipFile(options.dex_file, 'r') as dex_zip:
            for dex in (d for d in dex_zip.namelist() if d.endswith('.dex')):
              build_utils.AddToZipHermetic(out_apk, apk_dex_dir + dex,
                                           data=dex_zip.read(dex))
        elif options.dex_file:
          build_utils.AddToZipHermetic(out_apk, apk_dex_dir + 'classes.dex',
                                       src_path=options.dex_file)

        # 4. Native libraries.
        _AddNativeLibraries(out_apk,
                            native_libs,
                            options.android_abi,
                            options.uncompress_shared_libraries)

        if options.secondary_android_abi:
          _AddNativeLibraries(out_apk,
                              secondary_native_libs,
                              options.secondary_android_abi,
                              options.uncompress_shared_libraries)

        for name in sorted(options.native_lib_placeholders):
          # Note: Empty libs files are ignored by md5check (can cause issues
          # with stale builds when the only change is adding/removing
          # placeholders).
          apk_path = 'lib/%s/%s' % (options.android_abi, name)
          build_utils.AddToZipHermetic(out_apk, apk_path, data='')

        for name in sorted(options.secondary_native_lib_placeholders):
          # Note: Empty libs files are ignored by md5check (can cause issues
          # with stale builds when the only change is adding/removing
          # placeholders).
          apk_path = 'lib/%s/%s' % (options.secondary_android_abi, name)
          build_utils.AddToZipHermetic(out_apk, apk_path, data='')

        # 5. Resources
        for info in resource_infos[1:]:
          copy_resource(info)

        # 6. Java resources that should be accessible via
        # Class.getResourceAsStream(), in particular parts of Emma jar.
        # Prebuilt jars may contain class files which we shouldn't include.
        for java_resource in options.java_resources:
          with zipfile.ZipFile(java_resource, 'r') as java_resource_jar:
            for apk_path in java_resource_jar.namelist():
              apk_path_lower = apk_path.lower()

              if apk_path_lower.startswith('meta-inf/'):
                continue
              if apk_path_lower.endswith('/'):
                continue
              if apk_path_lower.endswith('.class'):
                continue

              build_utils.AddToZipHermetic(
                  out_apk, apk_root_dir + apk_path,
                  data=java_resource_jar.read(apk_path))

        if options.apk_pak_info_path:
          _MergePakInfoFiles(options.apk_pak_info_path, pak_infos)
        if options.apk_res_info_path:
          _MergeResInfoFiles(options.apk_res_info_path, options.resource_apk)

      if options.format == 'apk':
        finalize_apk.FinalizeApk(options.apksigner_path, options.zipalign_path,
                                 tmp_file, options.output_apk,
                                 options.key_path, options.key_passwd,
                                 options.key_name)
      else:
        shutil.move(tmp_file, options.output_apk)
        tmp_apk.delete = False

  build_utils.CallAndWriteDepfileIfStale(
      on_stale_md5,
      options,
      input_paths=input_paths + depfile_deps,
      input_strings=input_strings,
      output_paths=output_paths,
      depfile_deps=depfile_deps,
      add_pydeps=False)


if __name__ == '__main__':
  main(sys.argv[1:])
