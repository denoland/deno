#!/usr/bin/env python
#
# Copyright (c) 2012 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Compile Android resources into an intermediate APK.

This can also generate an R.txt, and an .srcjar file containing the proper
final R.java class for all resource packages the APK depends on.

This will crunch images with aapt2.
"""

import argparse
import collections
import multiprocessing.pool
import os
import re
import shutil
import subprocess
import sys
import zipfile
from xml.etree import ElementTree


from util import build_utils
from util import resource_utils

# Import jinja2 from third_party/jinja2
sys.path.insert(1, os.path.join(build_utils.DIR_SOURCE_ROOT, 'third_party'))
from jinja2 import Template # pylint: disable=F0401

# Pngs that we shouldn't convert to webp. Please add rationale when updating.
_PNG_WEBP_BLACKLIST_PATTERN = re.compile('|'.join([
    # Crashes on Galaxy S5 running L (https://crbug.com/807059).
    r'.*star_gray\.png',
    # Android requires pngs for 9-patch images.
    r'.*\.9\.png',
    # Daydream requires pngs for icon files.
    r'.*daydream_icon_.*\.png']))

# Regular expression for package declaration in 'aapt dump resources' output.
_RE_PACKAGE_DECLARATION = re.compile(
    r'^Package Group ([0-9]+) id=0x([0-9a-fA-F]+)')


def _PackageIdArgument(x):
  """Convert a string into a package ID while checking its range.

  Args:
    x: argument string.
  Returns:
    the package ID as an int, or -1 in case of error.
  """
  try:
    x = int(x, 0)
    if x < 0 or x > 127:
      x = -1
  except ValueError:
    x = -1
  return x


def _ParseArgs(args):
  """Parses command line options.

  Returns:
    An options object as from argparse.ArgumentParser.parse_args()
  """
  parser, input_opts, output_opts = resource_utils.ResourceArgsParser()

  input_opts.add_argument('--android-manifest', required=True,
                          help='AndroidManifest.xml path')

  input_opts.add_argument(
      '--shared-resources',
      action='store_true',
      help='Make all resources in R.java non-final and allow the resource IDs '
           'to be reset to a different package index when the apk is loaded by '
           'another application at runtime.')

  input_opts.add_argument(
      '--app-as-shared-lib',
      action='store_true',
      help='Same as --shared-resources, but also ensures all resource IDs are '
           'directly usable from the APK loaded as an application.')

  input_opts.add_argument(
      '--shared-resources-whitelist',
      help='An R.txt file acting as a whitelist for resources that should be '
           'non-final and have their package ID changed at runtime in R.java. '
           'Implies and overrides --shared-resources.')

  input_opts.add_argument('--proto-format', action='store_true',
                          help='Compile resources to protocol buffer format.')

  input_opts.add_argument('--support-zh-hk', action='store_true',
                          help='Use zh-rTW resources for zh-rHK.')

  input_opts.add_argument('--debuggable',
                          action='store_true',
                          help='Whether to add android:debuggable="true"')

  input_opts.add_argument('--version-code', help='Version code for apk.')
  input_opts.add_argument('--version-name', help='Version name for apk.')

  input_opts.add_argument(
      '--no-compress',
      help='disables compression for the given comma-separated list of '
           'extensions')

  input_opts.add_argument(
      '--locale-whitelist',
      default='[]',
      help='GN list of languages to include. All other language configs will '
          'be stripped out. List may include a combination of Android locales '
          'or Chrome locales.')

  input_opts.add_argument('--resource-blacklist-regex', default='',
                          help='Do not include matching drawables.')

  input_opts.add_argument(
      '--resource-blacklist-exceptions',
      default='[]',
      help='GN list of globs that say which blacklisted images to include even '
           'when --resource-blacklist-regex is set.')

  input_opts.add_argument('--png-to-webp', action='store_true',
                          help='Convert png files to webp format.')

  input_opts.add_argument('--webp-binary', default='',
                          help='Path to the cwebp binary.')

  input_opts.add_argument('--no-xml-namespaces',
                          action='store_true',
                          help='Whether to strip xml namespaces from processed '
                               'xml resources')

  input_opts.add_argument(
      '--check-resources-pkg-id', type=_PackageIdArgument,
      help='Check the package ID of the generated resources table. '
           'Value must be integer in [0..127] range.')

  output_opts.add_argument('--apk-path', required=True,
                           help='Path to output (partial) apk.')

  output_opts.add_argument('--apk-info-path', required=True,
                           help='Path to output info file for the partial apk.')

  output_opts.add_argument('--srcjar-out',
                           help='Path to srcjar to contain generated R.java.')

  output_opts.add_argument('--r-text-out',
                           help='Path to store the generated R.txt file.')

  output_opts.add_argument('--proguard-file',
                           help='Path to proguard.txt generated file')

  output_opts.add_argument(
      '--proguard-file-main-dex',
      help='Path to proguard.txt generated file for main dex')

  options = parser.parse_args(args)

  resource_utils.HandleCommonOptions(options)

  options.locale_whitelist = build_utils.ParseGnList(options.locale_whitelist)
  options.resource_blacklist_exceptions = build_utils.ParseGnList(
      options.resource_blacklist_exceptions)

  if options.check_resources_pkg_id is not None:
    if options.check_resources_pkg_id < 0:
      raise Exception(
          'Package resource id should be integer in [0..127] range.')

  if options.shared_resources and options.app_as_shared_lib:
    raise Exception('Only one of --app-as-shared-lib or --shared-resources '
                    'can be used.')

  return options


def _ExtractPackageIdFromApk(apk_path, aapt_path):
  """Extract the package ID of a given APK (even intermediate ones).

  Args:
    apk_path: Input apk path.
    aapt_path: Path to aapt tool.
  Returns:
    An integer corresponding to the APK's package id.
  Raises:
    Exception if there is no resources table in the input file.
  """
  cmd_args = [ aapt_path, 'dump', 'resources', apk_path ]
  output = build_utils.CheckOutput(cmd_args)

  for line in output.splitlines():
    m = _RE_PACKAGE_DECLARATION.match(line)
    if m:
      return int(m.group(2), 16)

  raise Exception("No resources in this APK!")


def _SortZip(original_path, sorted_path):
  """Generate new zip archive by sorting all files in the original by name."""
  with zipfile.ZipFile(sorted_path, 'w') as sorted_zip, \
      zipfile.ZipFile(original_path, 'r') as original_zip:
    for info in sorted(original_zip.infolist(), key=lambda i: i.filename):
      sorted_zip.writestr(info, original_zip.read(info))


def _IterFiles(root_dir):
  for root, _, files in os.walk(root_dir):
    for f in files:
      yield os.path.join(root, f)


def _DuplicateZhResources(resource_dirs):
  """Duplicate Taiwanese resources into Hong-Kong specific directory."""
  renamed_paths = dict()
  for resource_dir in resource_dirs:
    # We use zh-TW resources for zh-HK (if we have zh-TW resources).
    for path in _IterFiles(resource_dir):
      if 'zh-rTW' in path:
        hk_path = path.replace('zh-rTW', 'zh-rHK')
        build_utils.MakeDirectory(os.path.dirname(hk_path))
        shutil.copyfile(path, hk_path)
        renamed_paths[os.path.relpath(hk_path, resource_dir)] = os.path.relpath(
            path, resource_dir)
  return renamed_paths


def _ToAaptLocales(locale_whitelist, support_zh_hk):
  """Converts the list of Chrome locales to aapt config locales."""
  ret = set()
  for locale in locale_whitelist:
    locale = resource_utils.CHROME_TO_ANDROID_LOCALE_MAP.get(locale, locale)
    if locale is None or ('-' in locale and '-r' not in locale):
      raise Exception('CHROME_TO_ANDROID_LOCALE_MAP needs updating.'
                      ' Found: %s' % locale)
    ret.add(locale)
    # Always keep non-regional fall-backs.
    language = locale.split('-')[0]
    ret.add(language)

  # We don't actually support zh-HK in Chrome on Android, but we mimic the
  # native side behavior where we use zh-TW resources when the locale is set to
  # zh-HK. See https://crbug.com/780847.
  if support_zh_hk:
    assert not any('HK' in l for l in locale_whitelist), (
        'Remove special logic if zh-HK is now supported (crbug.com/780847).')
    ret.add('zh-rHK')
  return sorted(ret)


def _MoveImagesToNonMdpiFolders(res_root):
  """Move images from drawable-*-mdpi-* folders to drawable-* folders.

  Why? http://crbug.com/289843
  """
  renamed_paths = dict()
  for src_dir_name in os.listdir(res_root):
    src_components = src_dir_name.split('-')
    if src_components[0] != 'drawable' or 'mdpi' not in src_components:
      continue
    src_dir = os.path.join(res_root, src_dir_name)
    if not os.path.isdir(src_dir):
      continue
    dst_components = [c for c in src_components if c != 'mdpi']
    assert dst_components != src_components
    dst_dir_name = '-'.join(dst_components)
    dst_dir = os.path.join(res_root, dst_dir_name)
    build_utils.MakeDirectory(dst_dir)
    for src_file_name in os.listdir(src_dir):
      if not os.path.splitext(src_file_name)[1] in ('.png', '.webp'):
        continue
      src_file = os.path.join(src_dir, src_file_name)
      dst_file = os.path.join(dst_dir, src_file_name)
      assert not os.path.lexists(dst_file)
      shutil.move(src_file, dst_file)
      renamed_paths[os.path.relpath(dst_file, res_root)] = os.path.relpath(
          src_file, res_root)
  return renamed_paths


def _CreateLinkApkArgs(options):
  """Create command-line arguments list to invoke 'aapt2 link'.

  Args:
    options: The command-line options tuple.
  Returns:
    A list of strings corresponding to the command-line invokation for
    the command, matching the arguments from |options|.
  """
  link_command = [
    options.aapt2_path,
    'link',
    '--version-code', options.version_code,
    '--version-name', options.version_name,
    '--auto-add-overlay',
    '--no-version-vectors',
    '-o', options.apk_path,
  ]

  for j in options.include_resources:
    link_command += ['-I', j]
  if options.proguard_file:
    link_command += ['--proguard', options.proguard_file]
  if options.proguard_file_main_dex:
    link_command += ['--proguard-main-dex', options.proguard_file_main_dex]

  if options.no_compress:
    for ext in options.no_compress.split(','):
      link_command += ['-0', ext]

  # Note: only one of --proto-format, --shared-lib or --app-as-shared-lib
  #       can be used with recent versions of aapt2.
  if options.proto_format:
    link_command.append('--proto-format')
  elif options.shared_resources:
    link_command.append('--shared-lib')

  if options.locale_whitelist:
    aapt_locales = _ToAaptLocales(
        options.locale_whitelist, options.support_zh_hk)
    link_command += ['-c', ','.join(aapt_locales)]

  if options.no_xml_namespaces:
    link_command.append('--no-xml-namespaces')

  return link_command


def _ExtractVersionFromSdk(aapt_path, sdk_path):
  """Extract version code and name from Android SDK .jar file.

  Args:
    aapt_path: Path to 'aapt' build tool.
    sdk_path: Path to SDK-specific android.jar file.
  Returns:
    A (version_code, version_name) pair of strings.
  """
  output = build_utils.CheckOutput(
      [aapt_path, 'dump', 'badging', sdk_path],
      print_stdout=False, print_stderr=False)
  version_code = re.search(r"versionCode='(.*?)'", output).group(1)
  version_name = re.search(r"versionName='(.*?)'", output).group(1)
  return version_code, version_name,


def _FixManifest(options, temp_dir):
  """Fix the APK's AndroidManifest.xml.

  This adds any missing namespaces for 'android' and 'tools', and
  sets certains elements like 'platformBuildVersionCode' or
  'android:debuggable' depending on the content of |options|.

  Args:
    options: The command-line arguments tuple.
    temp_dir: A temporary directory where the fixed manifest will be written to.
  Returns:
    Path to the fixed manifest within |temp_dir|.
  """
  debug_manifest_path = os.path.join(temp_dir, 'AndroidManifest.xml')
  _ANDROID_NAMESPACE = 'http://schemas.android.com/apk/res/android'
  _TOOLS_NAMESPACE = 'http://schemas.android.com/tools'
  ElementTree.register_namespace('android', _ANDROID_NAMESPACE)
  ElementTree.register_namespace('tools', _TOOLS_NAMESPACE)
  original_manifest = ElementTree.parse(options.android_manifest)

  def maybe_extract_version(j):
    try:
      return _ExtractVersionFromSdk(options.aapt_path, j)
    except build_utils.CalledProcessError:
      return None

  android_sdk_jars = [j for j in options.include_resources
                      if os.path.basename(j) in ('android.jar',
                                                 'android_system.jar')]
  extract_all = [maybe_extract_version(j) for j in android_sdk_jars]
  successful_extractions = [x for x in extract_all if x]
  if len(successful_extractions) == 0:
    raise Exception(
        'Unable to find android SDK jar among candidates: %s'
            % ', '.join(android_sdk_jars))
  elif len(successful_extractions) > 1:
    raise Exception(
        'Found multiple android SDK jars among candidates: %s'
            % ', '.join(android_sdk_jars))
  version_code, version_name = successful_extractions.pop()

  # ElementTree.find does not work if the required tag is the root.
  if original_manifest.getroot().tag == 'manifest':
    manifest_node = original_manifest.getroot()
  else:
    manifest_node = original_manifest.find('manifest')

  manifest_node.set('platformBuildVersionCode', version_code)
  manifest_node.set('platformBuildVersionName', version_name)

  if options.debuggable:
    app_node = original_manifest.find('application')
    app_node.set('{%s}%s' % (_ANDROID_NAMESPACE, 'debuggable'), 'true')

  with open(debug_manifest_path, 'w') as debug_manifest:
    debug_manifest.write(ElementTree.tostring(
        original_manifest.getroot(), encoding='UTF-8'))

  return debug_manifest_path


def _ResourceNameFromPath(path):
  return os.path.splitext(os.path.basename(path))[0]


def _CreateKeepPredicate(resource_dirs, resource_blacklist_regex,
                         resource_blacklist_exceptions):
  """Return a predicate lambda to determine which resource files to keep."""
  if resource_blacklist_regex == '':
    # Do not extract dotfiles (e.g. ".gitkeep"). aapt ignores them anyways.
    return lambda path: os.path.basename(path)[0] != '.'

  # Returns False only for non-filtered, non-mipmap, non-whitelisted drawables.
  naive_predicate = lambda path: (
      not re.search(resource_blacklist_regex, path) or
      re.search(r'[/-]mipmap[/-]', path) or
      build_utils.MatchesGlob(path, resource_blacklist_exceptions))

  # Build a set of all non-filtered drawables to ensure that we never exclude
  # any drawable that does not exist in non-filtered densities.
  non_filtered_drawables = set()
  for resource_dir in resource_dirs:
    for path in _IterFiles(resource_dir):
      if re.search(r'[/-]drawable[/-]', path) and naive_predicate(path):
        non_filtered_drawables.add(_ResourceNameFromPath(path))

  return lambda path: (naive_predicate(path) or
      _ResourceNameFromPath(path) not in non_filtered_drawables)


def _ConvertToWebP(webp_binary, png_files):
  renamed_paths = dict()
  pool = multiprocessing.pool.ThreadPool(10)
  def convert_image(png_path_tuple):
    png_path, original_dir = png_path_tuple
    root = os.path.splitext(png_path)[0]
    webp_path = root + '.webp'
    args = [webp_binary, png_path, '-mt', '-quiet', '-m', '6', '-q', '100',
        '-lossless', '-o', webp_path]
    subprocess.check_call(args)
    os.remove(png_path)
    renamed_paths[os.path.relpath(webp_path, original_dir)] = os.path.relpath(
        png_path, original_dir)

  pool.map(convert_image, [f for f in png_files
                           if not _PNG_WEBP_BLACKLIST_PATTERN.match(f[0])])
  pool.close()
  pool.join()
  return renamed_paths


def _CompileDeps(aapt2_path, dep_subdirs, temp_dir):
  partials_dir = os.path.join(temp_dir, 'partials')
  build_utils.MakeDirectory(partials_dir)
  partial_compile_command = [
      aapt2_path,
      'compile',
      # TODO(wnwen): Turn this on once aapt2 forces 9-patch to be crunched.
      # '--no-crunch',
  ]
  pool = multiprocessing.pool.ThreadPool(10)
  def compile_partial(directory):
    dirname = os.path.basename(directory)
    partial_path = os.path.join(partials_dir, dirname + '.zip')
    compile_command = (partial_compile_command +
                       ['--dir', directory, '-o', partial_path])
    build_utils.CheckOutput(compile_command)

    # Sorting the files in the partial ensures deterministic output from the
    # aapt2 link step which uses order of files in the partial.
    sorted_partial_path = os.path.join(partials_dir, dirname + '.sorted.zip')
    _SortZip(partial_path, sorted_partial_path)

    return sorted_partial_path

  partials = pool.map(compile_partial, dep_subdirs)
  pool.close()
  pool.join()
  return partials


def _CreateResourceInfoFile(
    renamed_paths, apk_info_path, dependencies_res_zips):
  lines = set()
  for zip_file in dependencies_res_zips:
    zip_info_file_path = zip_file + '.info'
    if os.path.exists(zip_info_file_path):
      with open(zip_info_file_path, 'r') as zip_info_file:
        lines.update(zip_info_file.readlines())
  for dest, source in renamed_paths.iteritems():
    lines.add('Rename:{},{}\n'.format(dest, source))
  with open(apk_info_path, 'w') as info_file:
    info_file.writelines(sorted(lines))


def _PackageApk(options, dep_subdirs, temp_dir, gen_dir, r_txt_path):
  """Compile resources with aapt2 and generate intermediate .ap_ file.

  Args:
    options: The command-line options tuple. E.g. the generated apk
      will be written to |options.apk_path|.
    dep_subdirs: The list of directories where dependency resource zips
      were extracted (its content will be altered by this function).
    temp_dir: A temporary directory.
    gen_dir: Another temp directory where some intermediate files are
      generated.
    r_txt_path: The path where the R.txt file will written to.
  """
  renamed_paths = dict()
  renamed_paths.update(_DuplicateZhResources(dep_subdirs))

  keep_predicate = _CreateKeepPredicate(
      dep_subdirs, options.resource_blacklist_regex,
      options.resource_blacklist_exceptions)
  png_paths = []
  for directory in dep_subdirs:
    for f in _IterFiles(directory):
      if not keep_predicate(f):
        os.remove(f)
      elif f.endswith('.png'):
        png_paths.append((f, directory))
  if png_paths and options.png_to_webp:
    renamed_paths.update(_ConvertToWebP(options.webp_binary, png_paths))
  for directory in dep_subdirs:
    renamed_paths.update(_MoveImagesToNonMdpiFolders(directory))

  link_command = _CreateLinkApkArgs(options)
  link_command += ['--output-text-symbols', r_txt_path]
  # TODO(digit): Is this below actually required for R.txt generation?
  link_command += ['--java', gen_dir]

  fixed_manifest = _FixManifest(options, temp_dir)
  link_command += ['--manifest', fixed_manifest]

  partials = _CompileDeps(options.aapt2_path, dep_subdirs, temp_dir)
  for partial in partials:
    link_command += ['-R', partial]

  # Creates a .zip with AndroidManifest.xml, resources.arsc, res/*
  # Also creates R.txt
  build_utils.CheckOutput(
      link_command, print_stdout=False, print_stderr=False)
  _CreateResourceInfoFile(
      renamed_paths, options.apk_info_path, options.dependencies_res_zips)


def _WriteFinalRTxtFile(options, aapt_r_txt_path):
  """Determine final R.txt and return its location.

  This handles --r-text-in and --r-text-out options at the same time.

  Args:
    options: The command-line options tuple.
    aapt_r_txt_path: The path to the R.txt generated by aapt.
  Returns:
    Path to the final R.txt file.
  """
  if options.r_text_in:
    r_txt_file = options.r_text_in
  else:
    # When an empty res/ directory is passed, aapt does not write an R.txt.
    r_txt_file = aapt_r_txt_path
    if not os.path.exists(r_txt_file):
      build_utils.Touch(r_txt_file)

  if options.r_text_out:
    shutil.copyfile(r_txt_file, options.r_text_out)

  return r_txt_file


def _OnStaleMd5(options):
  with resource_utils.BuildContext() as build:
    dep_subdirs = resource_utils.ExtractDeps(options.dependencies_res_zips,
                                             build.deps_dir)

    _PackageApk(options, dep_subdirs, build.temp_dir, build.gen_dir,
                build.r_txt_path)

    r_txt_path = _WriteFinalRTxtFile(options, build.r_txt_path)

    # If --shared-resources-whitelist is used, the all resources listed in
    # the corresponding R.txt file will be non-final, and an onResourcesLoaded()
    # will be generated to adjust them at runtime.
    #
    # Otherwise, if --shared-resources is used, the all resources will be
    # non-final, and an onResourcesLoaded() method will be generated too.
    #
    # Otherwise, all resources will be final, and no method will be generated.
    #
    rjava_build_options = resource_utils.RJavaBuildOptions()
    if options.shared_resources_whitelist:
      rjava_build_options.ExportSomeResources(
          options.shared_resources_whitelist)
      rjava_build_options.GenerateOnResourcesLoaded()
    elif options.shared_resources or options.app_as_shared_lib:
      rjava_build_options.ExportAllResources()
      rjava_build_options.GenerateOnResourcesLoaded()

    resource_utils.CreateRJavaFiles(
        build.srcjar_dir, None, r_txt_path,
        options.extra_res_packages,
        options.extra_r_text_files,
        rjava_build_options)

    if options.srcjar_out:
      build_utils.ZipDir(options.srcjar_out, build.srcjar_dir)

    if options.check_resources_pkg_id is not None:
      expected_id = options.check_resources_pkg_id
      package_id = _ExtractPackageIdFromApk(options.apk_path,
                                            options.aapt_path)
      if package_id != expected_id:
        raise Exception('Invalid package ID 0x%x (expected 0x%x)' %
                        (package_id, expected_id))


def main(args):
  args = build_utils.ExpandFileArgs(args)
  options = _ParseArgs(args)

  # Order of these must match order specified in GN so that the correct one
  # appears first in the depfile.
  possible_output_paths = [
    options.apk_path,
    options.apk_path + '.info',
    options.r_text_out,
    options.srcjar_out,
    options.proguard_file,
    options.proguard_file_main_dex,
  ]
  output_paths = [x for x in possible_output_paths if x]

  # List python deps in input_strings rather than input_paths since the contents
  # of them does not change what gets written to the depsfile.
  input_strings = options.extra_res_packages + [
    options.shared_resources,
    options.resource_blacklist_regex,
    options.resource_blacklist_exceptions,
    str(options.debuggable),
    str(options.png_to_webp),
    str(options.support_zh_hk),
    str(options.no_xml_namespaces),
  ]

  input_strings.extend(_CreateLinkApkArgs(options))

  possible_input_paths = [
    options.aapt_path,
    options.aapt2_path,
    options.android_manifest,
    options.shared_resources_whitelist,
  ]
  possible_input_paths += options.include_resources
  input_paths = [x for x in possible_input_paths if x]
  input_paths.extend(options.dependencies_res_zips)
  input_paths.extend(options.extra_r_text_files)

  if options.webp_binary:
    input_paths.append(options.webp_binary)

  build_utils.CallAndWriteDepfileIfStale(
      lambda: _OnStaleMd5(options),
      options,
      input_paths=input_paths,
      input_strings=input_strings,
      output_paths=output_paths,
      depfile_deps=options.dependencies_res_zips + options.extra_r_text_files,
      add_pydeps=False)


if __name__ == '__main__':
  main(sys.argv[1:])
