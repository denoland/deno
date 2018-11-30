#!/usr/bin/env python
#
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import distutils.spawn
import itertools
import optparse
import os
import shutil
import re
import sys
import zipfile

from util import build_utils
from util import md5_check
from util import jar_info_utils

import jar

sys.path.append(
    os.path.join(build_utils.DIR_SOURCE_ROOT, 'third_party', 'colorama', 'src'))
import colorama


ERRORPRONE_WARNINGS_TO_TURN_OFF = [
  # TODO(crbug.com/834807): Follow steps in bug
  'DoubleBraceInitialization',
  # TODO(crbug.com/834790): Follow steps in bug.
  'CatchAndPrintStackTrace',
  # TODO(crbug.com/801210): Follow steps in bug.
  'SynchronizeOnNonFinalField',
  # TODO(crbug.com/802073): Follow steps in bug.
  'TypeParameterUnusedInFormals',
  # TODO(crbug.com/803484): Follow steps in bug.
  'CatchFail',
  # TODO(crbug.com/803485): Follow steps in bug.
  'JUnitAmbiguousTestClass',
  # Android platform default is always UTF-8.
  # https://developer.android.com/reference/java/nio/charset/Charset.html#defaultCharset()
  'DefaultCharset',
  # Low priority since the alternatives still work.
  'JdkObsolete',
  # We don't use that many lambdas.
  'FunctionalInterfaceClash',
  # There are lots of times when we just want to post a task.
  'FutureReturnValueIgnored',
  # Nice to be explicit about operators, but not necessary.
  'OperatorPrecedence',
  # Just false positives in our code.
  'ThreadJoinLoop',
  # Low priority corner cases with String.split.
  # Linking Guava and using Splitter was rejected
  # in the https://chromium-review.googlesource.com/c/chromium/src/+/871630.
  'StringSplitter',
  # Preferred to use another method since it propagates exceptions better.
  'ClassNewInstance',
  # Nice to have static inner classes but not necessary.
  'ClassCanBeStatic',
  # Explicit is better than implicit.
  'FloatCast',
  # Results in false positives.
  'ThreadLocalUsage',
  # Also just false positives.
  'Finally',
  # False positives for Chromium.
  'FragmentNotInstantiable',
  # Low priority to fix.
  'HidingField',
  # Low priority.
  'IntLongMath',
  # Low priority.
  'BadComparable',
  # Low priority.
  'EqualsHashCode',
  # Nice to fix but low priority.
  'TypeParameterShadowing',
  # Good to have immutable enums, also low priority.
  'ImmutableEnumChecker',
  # False positives for testing.
  'InputStreamSlowMultibyteRead',
  # Nice to have better primitives.
  'BoxedPrimitiveConstructor',
  # Not necessary for tests.
  'OverrideThrowableToString',
  # Nice to have better type safety.
  'CollectionToArraySafeParameter',
  # Makes logcat debugging more difficult, and does not provide obvious
  # benefits in the Chromium codebase.
  'ObjectToString',
]

ERRORPRONE_WARNINGS_TO_ERROR = [
  # Add warnings to this after fixing/suppressing all instances in our codebase.
  'ArgumentSelectionDefectChecker',
  'AssertionFailureIgnored',
  'FloatingPointLiteralPrecision',
  'JavaLangClash',
  'MissingFail',
  'MissingOverride',
  'NarrowingCompoundAssignment',
  'OrphanedFormatString',
  'ParameterName',
  'ParcelableCreator',
  'ReferenceEquality',
  'StaticGuardedByInstance',
  'StaticQualifiedUsingExpression',
  'UseCorrectAssertInTests',
]


def ProcessJavacOutput(output):
  fileline_prefix = r'(?P<fileline>(?P<file>[-.\w/\\]+.java):(?P<line>[0-9]+):)'
  warning_re = re.compile(
      fileline_prefix + r'(?P<full_message> warning: (?P<message>.*))$')
  error_re = re.compile(
      fileline_prefix + r'(?P<full_message> (?P<message>.*))$')
  marker_re = re.compile(r'\s*(?P<marker>\^)\s*$')

  # These warnings cannot be suppressed even for third party code. Deprecation
  # warnings especially do not help since we must support older android version.
  deprecated_re = re.compile(
      r'(Note: .* uses? or overrides? a deprecated API.)$')
  unchecked_re = re.compile(
      r'(Note: .* uses? unchecked or unsafe operations.)$')
  recompile_re = re.compile(r'(Note: Recompile with -Xlint:.* for details.)$')

  warning_color = ['full_message', colorama.Fore.YELLOW + colorama.Style.DIM]
  error_color = ['full_message', colorama.Fore.MAGENTA + colorama.Style.BRIGHT]
  marker_color = ['marker',  colorama.Fore.BLUE + colorama.Style.BRIGHT]

  def Colorize(line, regex, color):
    match = regex.match(line)
    start = match.start(color[0])
    end = match.end(color[0])
    return (line[:start]
            + color[1] + line[start:end]
            + colorama.Fore.RESET + colorama.Style.RESET_ALL
            + line[end:])

  def ApplyFilters(line):
    return not (deprecated_re.match(line)
        or unchecked_re.match(line)
        or recompile_re.match(line))

  def ApplyColors(line):
    if warning_re.match(line):
      line = Colorize(line, warning_re, warning_color)
    elif error_re.match(line):
      line = Colorize(line, error_re, error_color)
    elif marker_re.match(line):
      line = Colorize(line, marker_re, marker_color)
    return line

  return '\n'.join(map(ApplyColors, filter(ApplyFilters, output.split('\n'))))


def _ExtractClassFiles(jar_path, dest_dir, java_files):
  """Extracts all .class files not corresponding to |java_files|."""
  # Two challenges exist here:
  # 1. |java_files| have prefixes that are not represented in the the jar paths.
  # 2. A single .java file results in multiple .class files when it contains
  #    nested classes.
  # Here's an example:
  #   source path: ../../base/android/java/src/org/chromium/Foo.java
  #   jar paths: org/chromium/Foo.class, org/chromium/Foo$Inner.class
  # To extract only .class files not related to the given .java files, we strip
  # off ".class" and "$*.class" and use a substring match against java_files.
  def extract_predicate(path):
    if not path.endswith('.class'):
      return False
    path_without_suffix = re.sub(r'(?:\$|\.)[^/]*class$', '', path)
    partial_java_path = path_without_suffix + '.java'
    return not any(p.endswith(partial_java_path) for p in java_files)

  build_utils.ExtractAll(jar_path, path=dest_dir, predicate=extract_predicate)
  for path in build_utils.FindInDirectory(dest_dir, '*.class'):
    shutil.copystat(jar_path, path)


def _ConvertToJMakeArgs(javac_cmd, pdb_path):
  new_args = ['bin/jmake', '-pdb', pdb_path, '-jcexec', javac_cmd[0]]
  if md5_check.PRINT_EXPLANATIONS:
    new_args.append('-Xtiming')

  do_not_prefix = ('-classpath', '-bootclasspath')
  skip_next = False
  for arg in javac_cmd[1:]:
    if not skip_next and arg not in do_not_prefix:
      arg = '-C' + arg
    new_args.append(arg)
    skip_next = arg in do_not_prefix

  return new_args


def _ParsePackageAndClassNames(java_file):
  package_name = ''
  class_names = []
  with open(java_file) as f:
    for l in f:
      # Strip unindented comments.
      # Considers a leading * as a continuation of a multi-line comment (our
      # linter doesn't enforce a space before it like there should be).
      l = re.sub(r'^(?://.*|/?\*.*?(?:\*/\s*|$))', '', l)

      m = re.match(r'package\s+(.*?);', l)
      if m and not package_name:
        package_name = m.group(1)

      # Not exactly a proper parser, but works for sources that Chrome uses.
      # In order to not match nested classes, it just checks for lack of indent.
      m = re.match(r'(?:\S.*?)?(?:class|@?interface|enum)\s+(.+?)\b', l)
      if m:
        class_names.append(m.group(1))
  return package_name, class_names


def _CheckPathMatchesClassName(java_file, package_name, class_name):
  parts = package_name.split('.') + [class_name + '.java']
  expected_path_suffix = os.path.sep.join(parts)
  if not java_file.endswith(expected_path_suffix):
    raise Exception(('Java package+class name do not match its path.\n'
                     'Actual path: %s\nExpected path: %s') %
                    (java_file, expected_path_suffix))


def _CreateInfoFile(java_files, options, srcjar_files, javac_generated_sources):
  """Writes a .jar.info file.

  This maps fully qualified names for classes to either the java file that they
  are defined in or the path of the srcjar that they came from.

  For apks this also produces a coalesced .apk.jar.info file combining all the
  .jar.info files of its transitive dependencies.
  """
  info_data = dict()
  for java_file in itertools.chain(java_files, javac_generated_sources):
    package_name, class_names = _ParsePackageAndClassNames(java_file)
    for class_name in class_names:
      fully_qualified_name = '{}.{}'.format(package_name, class_name)
      info_data[fully_qualified_name] = java_file
    # Skip aidl srcjars since they don't indent code correctly.
    source = srcjar_files.get(java_file, java_file)
    if '_aidl.srcjar' in source:
      continue
    assert not options.chromium_code or len(class_names) == 1, (
        'Chromium java files must only have one class: {}'.format(source))
    if options.chromium_code:
      _CheckPathMatchesClassName(java_file, package_name, class_names[0])

  with build_utils.AtomicOutput(options.jar_path + '.info') as f:
    jar_info_utils.WriteJarInfoFile(f.name, info_data, srcjar_files)


def _OnStaleMd5(changes, options, javac_cmd, java_files, classpath_inputs,
                classpath):
  # Don't bother enabling incremental compilation for non-chromium code.
  incremental = options.incremental and options.chromium_code

  with build_utils.TempDir() as temp_dir:
    srcjars = options.java_srcjars

    classes_dir = os.path.join(temp_dir, 'classes')
    os.makedirs(classes_dir)

    changed_paths = None
    # jmake can handle deleted files, but it's a rare case and it would
    # complicate this script's logic.
    if incremental and changes.AddedOrModifiedOnly():
      changed_paths = set(changes.IterChangedPaths())
      # Do a full compile if classpath has changed.
      # jmake doesn't seem to do this on its own... Might be that ijars mess up
      # its change-detection logic.
      if any(p in changed_paths for p in classpath_inputs):
        changed_paths = None

    if options.incremental:
      pdb_path = options.jar_path + '.pdb'

    if incremental:
      # jmake is a compiler wrapper that figures out the minimal set of .java
      # files that need to be rebuilt given a set of .java files that have
      # changed.
      # jmake determines what files are stale based on timestamps between .java
      # and .class files. Since we use .jars, .srcjars, and md5 checks,
      # timestamp info isn't accurate for this purpose. Rather than use jmake's
      # programatic interface (like we eventually should), we ensure that all
      # .class files are newer than their .java files, and convey to jmake which
      # sources are stale by having their .class files be missing entirely
      # (by not extracting them).
      javac_cmd = _ConvertToJMakeArgs(javac_cmd, pdb_path)

    generated_java_dir = options.generated_dir
    # Incremental means not all files will be extracted, so don't bother
    # clearing out stale generated files.
    if not incremental:
      shutil.rmtree(generated_java_dir, True)

    srcjar_files = {}
    if srcjars:
      build_utils.MakeDirectory(generated_java_dir)
      jar_srcs = []
      for srcjar in options.java_srcjars:
        if changed_paths:
          changed_paths.update(os.path.join(generated_java_dir, f)
                               for f in changes.IterChangedSubpaths(srcjar))
        extracted_files = build_utils.ExtractAll(
            srcjar, no_clobber=not incremental, path=generated_java_dir,
            pattern='*.java')
        for path in extracted_files:
          # We want the path inside the srcjar so the viewer can have a tree
          # structure.
          srcjar_files[path] = '{}/{}'.format(
              srcjar, os.path.relpath(path, generated_java_dir))
        jar_srcs.extend(extracted_files)
      java_files.extend(jar_srcs)
      if changed_paths:
        # Set the mtime of all sources to 0 since we use the absence of .class
        # files to tell jmake which files are stale.
        for path in jar_srcs:
          os.utime(path, (0, 0))

    if java_files:
      if changed_paths:
        changed_java_files = [p for p in java_files if p in changed_paths]
        if os.path.exists(options.jar_path):
          _ExtractClassFiles(options.jar_path, classes_dir, changed_java_files)
        # Add the extracted files to the classpath. This is required because
        # when compiling only a subset of files, classes that haven't changed
        # need to be findable.
        classpath.append(classes_dir)

      # Can happen when a target goes from having no sources, to having sources.
      # It's created by the call to build_utils.Touch() below.
      if incremental:
        if os.path.exists(pdb_path) and not os.path.getsize(pdb_path):
          os.unlink(pdb_path)

      # Don't include the output directory in the initial set of args since it
      # being in a temp dir makes it unstable (breaks md5 stamping).
      cmd = javac_cmd + ['-d', classes_dir]

      # Pass classpath and source paths as response files to avoid extremely
      # long command lines that are tedius to debug.
      if classpath:
        cmd += ['-classpath', ':'.join(classpath)]

      java_files_rsp_path = os.path.join(temp_dir, 'files_list.txt')
      with open(java_files_rsp_path, 'w') as f:
        f.write(' '.join(java_files))
      cmd += ['@' + java_files_rsp_path]

      # JMake prints out some diagnostic logs that we want to ignore.
      # This assumes that all compiler output goes through stderr.
      stdout_filter = lambda s: ''
      if md5_check.PRINT_EXPLANATIONS:
        stdout_filter = None

      attempt_build = lambda: build_utils.CheckOutput(
          cmd,
          print_stdout=options.chromium_code,
          stdout_filter=stdout_filter,
          stderr_filter=ProcessJavacOutput)
      try:
        attempt_build()
      except build_utils.CalledProcessError as e:
        # Work-around for a bug in jmake (http://crbug.com/551449).
        if ('project database corrupted' not in e.output
            and 'jmake: internal Java exception' not in e.output):
          raise
        print ('Applying work-around for jmake project database corrupted '
               '(http://crbug.com/551449).')
        os.unlink(pdb_path)
        attempt_build()

    # Move any Annotation Processor-generated .java files into $out/gen
    # so that codesearch can find them.
    javac_generated_sources = []
    for src_path in build_utils.FindInDirectory(classes_dir, '*.java'):
      dst_path = os.path.join(
          generated_java_dir, os.path.relpath(src_path, classes_dir))
      build_utils.MakeDirectory(os.path.dirname(dst_path))
      shutil.move(src_path, dst_path)
      javac_generated_sources.append(dst_path)

    _CreateInfoFile(java_files, options, srcjar_files, javac_generated_sources)

    if options.incremental and (not java_files or not incremental):
      # Make sure output exists.
      build_utils.Touch(pdb_path)

    with build_utils.AtomicOutput(options.jar_path) as f:
      jar.JarDirectory(classes_dir,
                       f.name,
                       provider_configurations=options.provider_configurations,
                       additional_files=options.additional_jar_files)


def _ParseAndFlattenGnLists(gn_lists):
  ret = []
  for arg in gn_lists:
    ret.extend(build_utils.ParseGnList(arg))
  return ret


def _ParseOptions(argv):
  parser = optparse.OptionParser()
  build_utils.AddDepfileOption(parser)

  parser.add_option(
      '--java-srcjars',
      action='append',
      default=[],
      help='List of srcjars to include in compilation.')
  parser.add_option(
      '--generated-dir',
      help='Subdirectory within target_gen_dir to place extracted srcjars and '
           'annotation processor output for codesearch to find.')
  parser.add_option(
      '--bootclasspath',
      action='append',
      default=[],
      help='Boot classpath for javac. If this is specified multiple times, '
      'they will all be appended to construct the classpath.')
  parser.add_option(
      '--java-version',
      help='Java language version to use in -source and -target args to javac.')
  parser.add_option(
      '--full-classpath',
      action='append',
      help='Classpath to use when annotation processors are present.')
  parser.add_option(
      '--interface-classpath',
      action='append',
      help='Classpath to use when no annotation processors are present.')
  parser.add_option(
      '--incremental',
      action='store_true',
      help='Whether to re-use .class files rather than recompiling them '
           '(when possible).')
  parser.add_option(
      '--processors',
      action='append',
      help='GN list of annotation processor main classes.')
  parser.add_option(
      '--processorpath',
      action='append',
      help='GN list of jars that comprise the classpath used for Annotation '
           'Processors.')
  parser.add_option(
      '--processor-arg',
      dest='processor_args',
      action='append',
      help='key=value arguments for the annotation processors.')
  parser.add_option(
      '--provider-configuration',
      dest='provider_configurations',
      action='append',
      help='File to specify a service provider. Will be included '
           'in the jar under META-INF/services.')
  parser.add_option(
      '--additional-jar-file',
      dest='additional_jar_files',
      action='append',
      help='Additional files to package into jar. By default, only Java .class '
           'files are packaged into the jar. Files should be specified in '
           'format <filename>:<path to be placed in jar>.')
  parser.add_option(
      '--chromium-code',
      type='int',
      help='Whether code being compiled should be built with stricter '
      'warnings for chromium code.')
  parser.add_option(
      '--use-errorprone-path',
      help='Use the Errorprone compiler at this path.')
  parser.add_option('--jar-path', help='Jar output path.')
  parser.add_option(
      '--javac-arg',
      action='append',
      default=[],
      help='Additional arguments to pass to javac.')

  options, args = parser.parse_args(argv)
  build_utils.CheckOptions(options, parser, required=('jar_path',))

  options.bootclasspath = _ParseAndFlattenGnLists(options.bootclasspath)
  options.full_classpath = _ParseAndFlattenGnLists(options.full_classpath)
  options.interface_classpath = _ParseAndFlattenGnLists(
      options.interface_classpath)
  options.processorpath = _ParseAndFlattenGnLists(options.processorpath)
  options.processors = _ParseAndFlattenGnLists(options.processors)
  options.java_srcjars = _ParseAndFlattenGnLists(options.java_srcjars)

  if options.java_version == '1.8' and options.bootclasspath:
    # Android's boot jar doesn't contain all java 8 classes.
    # See: https://github.com/evant/gradle-retrolambda/issues/23.
    # Get the path of the jdk folder by searching for the 'jar' executable. We
    # cannot search for the 'javac' executable because goma provides a custom
    # version of 'javac'.
    jar_path = os.path.realpath(distutils.spawn.find_executable('jar'))
    jdk_dir = os.path.dirname(os.path.dirname(jar_path))
    rt_jar = os.path.join(jdk_dir, 'jre', 'lib', 'rt.jar')
    options.bootclasspath.append(rt_jar)

  additional_jar_files = []
  for arg in options.additional_jar_files or []:
    filepath, jar_filepath = arg.split(':')
    additional_jar_files.append((filepath, jar_filepath))
  options.additional_jar_files = additional_jar_files

  java_files = []
  for arg in args:
    # Interpret a path prefixed with @ as a file containing a list of sources.
    if arg.startswith('@'):
      java_files.extend(build_utils.ReadSourcesList(arg[1:]))
    else:
      java_files.append(arg)

  return options, java_files


def main(argv):
  colorama.init()

  argv = build_utils.ExpandFileArgs(argv)
  options, java_files = _ParseOptions(argv)

  if options.use_errorprone_path:
    javac_path = options.use_errorprone_path
  else:
    javac_path = distutils.spawn.find_executable('javac')
  javac_cmd = [javac_path]

  javac_cmd.extend((
    '-g',
    # Chromium only allows UTF8 source files.  Being explicit avoids
    # javac pulling a default encoding from the user's environment.
    '-encoding', 'UTF-8',
    # Prevent compiler from compiling .java files not listed as inputs.
    # See: http://blog.ltgt.net/most-build-tools-misuse-javac/
    '-sourcepath', ':',
  ))

  if options.use_errorprone_path:
    for warning in ERRORPRONE_WARNINGS_TO_TURN_OFF:
      javac_cmd.append('-Xep:{}:OFF'.format(warning))
    for warning in ERRORPRONE_WARNINGS_TO_ERROR:
      javac_cmd.append('-Xep:{}:ERROR'.format(warning))

  if options.java_version:
    javac_cmd.extend([
      '-source', options.java_version,
      '-target', options.java_version,
    ])

  if options.chromium_code:
    javac_cmd.extend(['-Xlint:unchecked', '-Werror'])
  else:
    # XDignore.symbol.file makes javac compile against rt.jar instead of
    # ct.sym. This means that using a java internal package/class will not
    # trigger a compile warning or error.
    javac_cmd.extend(['-XDignore.symbol.file'])

  if options.processors:
    javac_cmd.extend(['-processor', ','.join(options.processors)])

  if options.bootclasspath:
    javac_cmd.extend(['-bootclasspath', ':'.join(options.bootclasspath)])

  # Annotation processors crash when given interface jars.
  active_classpath = (
      options.full_classpath
      if options.processors else options.interface_classpath)
  classpath = []
  if active_classpath:
    classpath.extend(active_classpath)

  if options.processorpath:
    javac_cmd.extend(['-processorpath', ':'.join(options.processorpath)])
  if options.processor_args:
    for arg in options.processor_args:
      javac_cmd.extend(['-A%s' % arg])

  javac_cmd.extend(options.javac_arg)

  classpath_inputs = (options.bootclasspath + options.interface_classpath +
                      options.processorpath)
  # GN already knows of java_files, so listing them just make things worse when
  # they change.
  depfile_deps = ([javac_path] + classpath_inputs + options.java_srcjars)
  input_paths = depfile_deps + java_files

  output_paths = [
      options.jar_path,
      options.jar_path + '.info',
  ]
  if options.incremental:
    output_paths.append(options.jar_path + '.pdb')

  # An escape hatch to be able to check if incremental compiles are causing
  # problems.
  force = int(os.environ.get('DISABLE_INCREMENTAL_JAVAC', 0))

  # List python deps in input_strings rather than input_paths since the contents
  # of them does not change what gets written to the depsfile.
  build_utils.CallAndWriteDepfileIfStale(
      lambda changes: _OnStaleMd5(changes, options, javac_cmd, java_files,
                                  classpath_inputs, classpath),
      options,
      depfile_deps=depfile_deps,
      input_paths=input_paths,
      input_strings=javac_cmd + classpath,
      output_paths=output_paths,
      force=force,
      pass_changes=True,
      add_pydeps=False)


if __name__ == '__main__':
  sys.exit(main(sys.argv[1:]))
