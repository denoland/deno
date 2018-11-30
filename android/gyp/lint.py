#!/usr/bin/env python
#
# Copyright (c) 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Runs Android's lint tool."""


import argparse
import os
import re
import sys
import traceback
from xml.dom import minidom

from util import build_utils

_LINT_MD_URL = 'https://chromium.googlesource.com/chromium/src/+/master/build/android/docs/lint.md' # pylint: disable=line-too-long


def _OnStaleMd5(lint_path, config_path, processed_config_path,
                manifest_path, result_path, product_dir, sources, jar_path,
                cache_dir, android_sdk_version, srcjars, resource_sources,
                disable=None, classpath=None, can_fail_build=False,
                include_unexpected=False, silent=False):
  def _RebasePath(path):
    """Returns relative path to top-level src dir.

    Args:
      path: A path relative to cwd.
    """
    ret = os.path.relpath(os.path.abspath(path), build_utils.DIR_SOURCE_ROOT)
    # If it's outside of src/, just use abspath.
    if ret.startswith('..'):
      ret = os.path.abspath(path)
    return ret

  def _ProcessConfigFile():
    if not config_path or not processed_config_path:
      return
    if not build_utils.IsTimeStale(processed_config_path, [config_path]):
      return

    with open(config_path, 'rb') as f:
      content = f.read().replace(
          'PRODUCT_DIR', _RebasePath(product_dir))

    with open(processed_config_path, 'wb') as f:
      f.write(content)

  def _ProcessResultFile():
    with open(result_path, 'rb') as f:
      content = f.read().replace(
          _RebasePath(product_dir), 'PRODUCT_DIR')

    with open(result_path, 'wb') as f:
      f.write(content)

  def _ParseAndShowResultFile():
    dom = minidom.parse(result_path)
    issues = dom.getElementsByTagName('issue')
    if not silent:
      print >> sys.stderr
      for issue in issues:
        issue_id = issue.attributes['id'].value
        message = issue.attributes['message'].value
        location_elem = issue.getElementsByTagName('location')[0]
        path = location_elem.attributes['file'].value
        line = location_elem.getAttribute('line')
        if line:
          error = '%s:%s %s: %s [warning]' % (path, line, message, issue_id)
        else:
          # Issues in class files don't have a line number.
          error = '%s %s: %s [warning]' % (path, message, issue_id)
        print >> sys.stderr, error.encode('utf-8')
        for attr in ['errorLine1', 'errorLine2']:
          error_line = issue.getAttribute(attr)
          if error_line:
            print >> sys.stderr, error_line.encode('utf-8')
    return len(issues)

  with build_utils.TempDir() as temp_dir:
    _ProcessConfigFile()

    cmd = [
        _RebasePath(lint_path), '-Werror', '--exitcode', '--showall',
        '--xml', _RebasePath(result_path),
    ]
    if jar_path:
      # --classpath is just for .class files for this one target.
      cmd.extend(['--classpath', _RebasePath(jar_path)])
    if processed_config_path:
      cmd.extend(['--config', _RebasePath(processed_config_path)])

    tmp_dir_counter = [0]
    def _NewTempSubdir(prefix, append_digit=True):
      # Helper function to create a new sub directory based on the number of
      # subdirs created earlier.
      if append_digit:
        tmp_dir_counter[0] += 1
        prefix += str(tmp_dir_counter[0])
      new_dir = os.path.join(temp_dir, prefix)
      os.makedirs(new_dir)
      return new_dir

    resource_dirs = []
    for resource_source in resource_sources:
      if os.path.isdir(resource_source):
        resource_dirs.append(resource_source)
      else:
        # This is a zip file with generated resources (e. g. strings from GRD).
        # Extract it to temporary folder.
        resource_dir = _NewTempSubdir(resource_source, append_digit=False)
        resource_dirs.append(resource_dir)
        build_utils.ExtractAll(resource_source, path=resource_dir)

    for resource_dir in resource_dirs:
      cmd.extend(['--resources', _RebasePath(resource_dir)])

    if classpath:
      # --libraries is the classpath (excluding active target).
      cp = ':'.join(_RebasePath(p) for p in classpath)
      cmd.extend(['--libraries', cp])

    # There may be multiple source files with the same basename (but in
    # different directories). It is difficult to determine what part of the path
    # corresponds to the java package, and so instead just link the source files
    # into temporary directories (creating a new one whenever there is a name
    # conflict).
    def PathInDir(d, src):
      subpath = os.path.join(d, _RebasePath(src))
      subdir = os.path.dirname(subpath)
      if not os.path.exists(subdir):
        os.makedirs(subdir)
      return subpath

    src_dirs = []
    for src in sources:
      src_dir = None
      for d in src_dirs:
        if not os.path.exists(PathInDir(d, src)):
          src_dir = d
          break
      if not src_dir:
        src_dir = _NewTempSubdir('SRC_ROOT')
        src_dirs.append(src_dir)
        cmd.extend(['--sources', _RebasePath(src_dir)])
      os.symlink(os.path.abspath(src), PathInDir(src_dir, src))

    if srcjars:
      srcjar_paths = build_utils.ParseGnList(srcjars)
      if srcjar_paths:
        srcjar_dir = _NewTempSubdir('SRC_ROOT')
        cmd.extend(['--sources', _RebasePath(srcjar_dir)])
        for srcjar in srcjar_paths:
          build_utils.ExtractAll(srcjar, path=srcjar_dir)

    if disable:
      cmd.extend(['--disable', ','.join(disable)])

    project_dir = _NewTempSubdir('SRC_ROOT')
    if android_sdk_version:
      # Create dummy project.properies file in a temporary "project" directory.
      # It is the only way to add Android SDK to the Lint's classpath. Proper
      # classpath is necessary for most source-level checks.
      with open(os.path.join(project_dir, 'project.properties'), 'w') \
          as propfile:
        print >> propfile, 'target=android-{}'.format(android_sdk_version)

    # Put the manifest in a temporary directory in order to avoid lint detecting
    # sibling res/ and src/ directories (which should be pass explicitly if they
    # are to be included).
    if not manifest_path:
      manifest_path = os.path.join(
          build_utils.DIR_SOURCE_ROOT, 'build', 'android',
          'AndroidManifest.xml')
    os.symlink(os.path.abspath(manifest_path),
               os.path.join(project_dir, 'AndroidManifest.xml'))
    cmd.append(project_dir)

    if os.path.exists(result_path):
      os.remove(result_path)

    env = os.environ.copy()
    stderr_filter = None
    if cache_dir:
      env['_JAVA_OPTIONS'] = '-Duser.home=%s' % _RebasePath(cache_dir)
      # When _JAVA_OPTIONS is set, java prints to stderr:
      # Picked up _JAVA_OPTIONS: ...
      #
      # We drop all lines that contain _JAVA_OPTIONS from the output
      stderr_filter = lambda l: re.sub(r'.*_JAVA_OPTIONS.*\n?', '', l)

    def fail_func(returncode, stderr):
      if returncode != 0:
        return True
      if (include_unexpected and
          'Unexpected failure during lint analysis' in stderr):
        return True
      return False

    try:
      build_utils.CheckOutput(cmd, cwd=build_utils.DIR_SOURCE_ROOT,
                              env=env or None, stderr_filter=stderr_filter,
                              fail_func=fail_func)
    except build_utils.CalledProcessError:
      # There is a problem with lint usage
      if not os.path.exists(result_path):
        raise

      # Sometimes produces empty (almost) files:
      if os.path.getsize(result_path) < 10:
        if can_fail_build:
          raise
        elif not silent:
          traceback.print_exc()
        return

      # There are actual lint issues
      try:
        num_issues = _ParseAndShowResultFile()
      except Exception: # pylint: disable=broad-except
        if not silent:
          print 'Lint created unparseable xml file...'
          print 'File contents:'
          with open(result_path) as f:
            print f.read()
        if not can_fail_build:
          return

      if can_fail_build and not silent:
        traceback.print_exc()

      # There are actual lint issues
      try:
        num_issues = _ParseAndShowResultFile()
      except Exception: # pylint: disable=broad-except
        if not silent:
          print 'Lint created unparseable xml file...'
          print 'File contents:'
          with open(result_path) as f:
            print f.read()
        raise

      _ProcessResultFile()
      if num_issues == 0 and include_unexpected:
        msg = 'Please refer to output above for unexpected lint failures.\n'
      else:
        msg = ('\nLint found %d new issues.\n'
               ' - For full explanation, please refer to %s\n'
               ' - For more information about lint and how to fix lint issues,'
               ' please refer to %s\n' %
               (num_issues, _RebasePath(result_path), _LINT_MD_URL))
      if not silent:
        print >> sys.stderr, msg
      if can_fail_build:
        raise Exception('Lint failed.')


def _FindInDirectories(directories, filename_filter):
  all_files = []
  for directory in directories:
    all_files.extend(build_utils.FindInDirectory(directory, filename_filter))
  return all_files


def main():
  parser = argparse.ArgumentParser()
  build_utils.AddDepfileOption(parser)

  parser.add_argument('--lint-path', required=True,
                      help='Path to lint executable.')
  parser.add_argument('--product-dir', required=True,
                      help='Path to product dir.')
  parser.add_argument('--result-path', required=True,
                      help='Path to XML lint result file.')
  parser.add_argument('--cache-dir', required=True,
                      help='Path to the directory in which the android cache '
                           'directory tree should be stored.')
  parser.add_argument('--platform-xml-path', required=True,
                      help='Path to api-platforms.xml')
  parser.add_argument('--android-sdk-version',
                      help='Version (API level) of the Android SDK used for '
                           'building.')
  parser.add_argument('--create-cache', action='store_true',
                      help='Mark the lint cache file as an output rather than '
                      'an input.')
  parser.add_argument('--can-fail-build', action='store_true',
                      help='If set, script will exit with nonzero exit status'
                           ' if lint errors are present')
  parser.add_argument('--include-unexpected-failures', action='store_true',
                      help='If set, script will exit with nonzero exit status'
                           ' if lint itself crashes with unexpected failures.')
  parser.add_argument('--config-path',
                      help='Path to lint suppressions file.')
  parser.add_argument('--disable',
                      help='List of checks to disable.')
  parser.add_argument('--jar-path',
                      help='Jar file containing class files.')
  parser.add_argument('--java-sources-file',
                      help='File containing a list of java files.')
  parser.add_argument('--manifest-path',
                      help='Path to AndroidManifest.xml')
  parser.add_argument('--classpath', default=[], action='append',
                      help='GYP-list of classpath .jar files')
  parser.add_argument('--processed-config-path',
                      help='Path to processed lint suppressions file.')
  parser.add_argument('--resource-dir',
                      help='Path to resource dir.')
  parser.add_argument('--resource-sources', default=[], action='append',
                      help='GYP-list of resource sources (directories with '
                      'resources or archives created by resource-generating '
                      'tasks.')
  parser.add_argument('--silent', action='store_true',
                      help='If set, script will not log anything.')
  parser.add_argument('--src-dirs',
                      help='Directories containing java files.')
  parser.add_argument('--srcjars',
                      help='GN list of included srcjars.')

  args = parser.parse_args(build_utils.ExpandFileArgs(sys.argv[1:]))

  sources = []
  if args.src_dirs:
    src_dirs = build_utils.ParseGnList(args.src_dirs)
    sources = _FindInDirectories(src_dirs, '*.java')
  elif args.java_sources_file:
    sources.extend(build_utils.ReadSourcesList(args.java_sources_file))

  if args.config_path and not args.processed_config_path:
    parser.error('--config-path specified without --processed-config-path')
  elif args.processed_config_path and not args.config_path:
    parser.error('--processed-config-path specified without --config-path')

  input_paths = [
      args.lint_path,
      args.platform_xml_path,
  ]
  if args.config_path:
    input_paths.append(args.config_path)
  if args.jar_path:
    input_paths.append(args.jar_path)
  if args.manifest_path:
    input_paths.append(args.manifest_path)
  if sources:
    input_paths.extend(sources)
  classpath = []
  for gyp_list in args.classpath:
    classpath.extend(build_utils.ParseGnList(gyp_list))
  input_paths.extend(classpath)

  resource_sources = []
  if args.resource_dir:
    # Backward compatibility with GYP
    resource_sources += [ args.resource_dir ]

  for gyp_list in args.resource_sources:
    resource_sources += build_utils.ParseGnList(gyp_list)

  for resource_source in resource_sources:
    if os.path.isdir(resource_source):
      input_paths.extend(build_utils.FindInDirectory(resource_source, '*'))
    else:
      input_paths.append(resource_source)

  input_strings = [
    args.can_fail_build,
    args.include_unexpected_failures,
    args.silent,
  ]
  if args.android_sdk_version:
    input_strings.append(args.android_sdk_version)
  if args.processed_config_path:
    input_strings.append(args.processed_config_path)

  disable = []
  if args.disable:
    disable = build_utils.ParseGnList(args.disable)
    input_strings.extend(disable)

  output_paths = [ args.result_path ]

  build_utils.CallAndWriteDepfileIfStale(
      lambda: _OnStaleMd5(args.lint_path,
                          args.config_path,
                          args.processed_config_path,
                          args.manifest_path, args.result_path,
                          args.product_dir, sources,
                          args.jar_path,
                          args.cache_dir,
                          args.android_sdk_version,
                          args.srcjars,
                          resource_sources,
                          disable=disable,
                          classpath=classpath,
                          can_fail_build=args.can_fail_build,
                          include_unexpected=args.include_unexpected_failures,
                          silent=args.silent),
      args,
      input_paths=input_paths,
      input_strings=input_strings,
      output_paths=output_paths,
      depfile_deps=classpath,
      add_pydeps=False)


if __name__ == '__main__':
  sys.exit(main())
