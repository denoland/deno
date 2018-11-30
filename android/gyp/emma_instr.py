#!/usr/bin/env python
#
# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Instruments classes and jar files.

This script corresponds to the 'emma_instr' action in the java build process.
Depending on whether emma_instrument is set, the 'emma_instr' action will either
call the instrument command or the copy command.

Possible commands are:
- instrument_jar: Accepts a jar and instruments it using emma.jar.
- copy: Called when EMMA coverage is not enabled. This allows us to make
      this a required step without necessarily instrumenting on every build.
      Also removes any stale coverage files.
"""

import collections
import json
import optparse
import os
import shutil
import sys
import tempfile

from util import build_utils


def _AddCommonOptions(option_parser):
  """Adds common options to |option_parser|."""
  option_parser.add_option('--input-path',
                           help=('Path to input file(s). Either the classes '
                                 'directory, or the path to a jar.'))
  option_parser.add_option('--output-path',
                           help=('Path to output final file(s) to. Either the '
                                 'final classes directory, or the directory in '
                                 'which to place the instrumented/copied jar.'))
  option_parser.add_option('--coverage-file',
                           help='File to create with coverage metadata.')
  option_parser.add_option('--sources-list-file',
                           help='File to create with the list of sources.')


def _AddInstrumentOptions(option_parser):
  """Adds options related to instrumentation to |option_parser|."""
  _AddCommonOptions(option_parser)
  option_parser.add_option('--source-dirs',
                           help='Space separated list of source directories. '
                                'source-files should not be specified if '
                                'source-dirs is specified')
  option_parser.add_option('--source-files',
                           help='Space separated list of source files. '
                                'source-dirs should not be specified if '
                                'source-files is specified')
  option_parser.add_option('--java-sources-file',
                           help='File containing newline-separated .java paths')
  option_parser.add_option('--src-root',
                           help='Root of the src repository.')
  option_parser.add_option('--emma-jar',
                           help='Path to emma.jar.')
  option_parser.add_option(
      '--filter-string', default='',
      help=('Filter string consisting of a list of inclusion/exclusion '
            'patterns separated with whitespace and/or comma.'))


def _RunCopyCommand(_command, options, _, option_parser):
  """Copies the jar from input to output locations.

  Also removes any old coverage/sources file.

  Args:
    command: String indicating the command that was received to trigger
        this function.
    options: optparse options dictionary.
    args: List of extra args from optparse.
    option_parser: optparse.OptionParser object.

  Returns:
    An exit code.
  """
  if not (options.input_path and options.output_path and
          options.coverage_file and options.sources_list_file):
    option_parser.error('All arguments are required.')

  if os.path.exists(options.coverage_file):
    os.remove(options.coverage_file)
  if os.path.exists(options.sources_list_file):
    os.remove(options.sources_list_file)

  shutil.copy(options.input_path, options.output_path)


def _GetSourceDirsFromSourceFiles(source_files):
  """Returns list of directories for the files in |source_files|.

  Args:
    source_files: List of source files.

  Returns:
    List of source directories.
  """
  return list(set(os.path.dirname(source_file) for source_file in source_files))


def _CreateSourcesListFile(source_dirs, sources_list_file, src_root):
  """Adds all normalized source directories to |sources_list_file|.

  Args:
    source_dirs: List of source directories.
    sources_list_file: File into which to write the JSON list of sources.
    src_root: Root which sources added to the file should be relative to.

  Returns:
    An exit code.
  """
  src_root = os.path.abspath(src_root)
  relative_sources = []
  for s in source_dirs:
    abs_source = os.path.abspath(s)
    if abs_source[:len(src_root)] != src_root:
      print ('Error: found source directory not under repository root: %s %s'
             % (abs_source, src_root))
      return 1
    rel_source = os.path.relpath(abs_source, src_root)

    relative_sources.append(rel_source)

  with open(sources_list_file, 'w') as f:
    json.dump(relative_sources, f)


def _RunInstrumentCommand(_command, options, _, option_parser):
  """Instruments jar files using EMMA.

  Args:
    command: String indicating the command that was received to trigger
        this function.
    options: optparse options dictionary.
    args: List of extra args from optparse.
    option_parser: optparse.OptionParser object.

  Returns:
    An exit code.
  """
  if not (options.input_path and options.output_path and
          options.coverage_file and options.sources_list_file and
          (options.source_files or options.source_dirs or
           options.java_sources_file) and
          options.src_root and options.emma_jar):
    option_parser.error('All arguments are required.')

  if os.path.exists(options.coverage_file):
    os.remove(options.coverage_file)
  temp_dir = tempfile.mkdtemp()
  try:
    cmd = ['java', '-cp', options.emma_jar,
           'emma', 'instr',
           '-ip', options.input_path,
           '-ix', options.filter_string,
           '-d', temp_dir,
           '-out', options.coverage_file,
           '-m', 'fullcopy']
    build_utils.CheckOutput(cmd)

    # File is not generated when filter_string doesn't match any files.
    if not os.path.exists(options.coverage_file):
      build_utils.Touch(options.coverage_file)

    temp_jar_dir = os.path.join(temp_dir, 'lib')
    jars = os.listdir(temp_jar_dir)
    if len(jars) != 1:
      print('Error: multiple output files in: %s' % (temp_jar_dir))
      return 1

    # Delete output_path first to avoid modifying input_path in the case where
    # input_path is a hardlink to output_path. http://crbug.com/571642
    if os.path.exists(options.output_path):
      os.unlink(options.output_path)
    shutil.move(os.path.join(temp_jar_dir, jars[0]), options.output_path)
  finally:
    shutil.rmtree(temp_dir)

  if options.source_dirs:
    source_dirs = build_utils.ParseGnList(options.source_dirs)
  else:
    source_files = []
    if options.source_files:
      source_files += build_utils.ParseGnList(options.source_files)
    if options.java_sources_file:
      source_files.extend(
          build_utils.ReadSourcesList(options.java_sources_file))
    source_dirs = _GetSourceDirsFromSourceFiles(source_files)

  # TODO(GYP): In GN, we are passed the list of sources, detecting source
  # directories, then walking them to re-establish the list of sources.
  # This can obviously be simplified!
  _CreateSourcesListFile(source_dirs, options.sources_list_file,
                         options.src_root)

  return 0


CommandFunctionTuple = collections.namedtuple(
    'CommandFunctionTuple', ['add_options_func', 'run_command_func'])
VALID_COMMANDS = {
    'copy': CommandFunctionTuple(_AddCommonOptions,
                                 _RunCopyCommand),
    'instrument_jar': CommandFunctionTuple(_AddInstrumentOptions,
                                           _RunInstrumentCommand),
}


class CommandOptionParser(optparse.OptionParser):
  """Wrapper class for OptionParser to help with listing commands."""

  def __init__(self, *args, **kwargs):
    """Creates a CommandOptionParser.

    Args:
      commands_dict: A dictionary mapping command strings to an object defining
          - add_options_func: Adds options to the option parser
          - run_command_func: Runs the command itself.
      example: An example command.
      everything else: Passed to optparse.OptionParser contructor.
    """
    self.commands_dict = kwargs.pop('commands_dict', {})
    self.example = kwargs.pop('example', '')
    if not 'usage' in kwargs:
      kwargs['usage'] = 'Usage: %prog <command> [options]'
    optparse.OptionParser.__init__(self, *args, **kwargs)

  #override
  def get_usage(self):
    normal_usage = optparse.OptionParser.get_usage(self)
    command_list = self.get_command_list()
    example = self.get_example()
    return self.expand_prog_name(normal_usage + example + command_list)

  #override
  def get_command_list(self):
    if self.commands_dict.keys():
      return '\nCommands:\n  %s\n' % '\n  '.join(
          sorted(self.commands_dict.keys()))
    return ''

  def get_example(self):
    if self.example:
      return '\nExample:\n  %s\n' % self.example
    return ''


def main():
  option_parser = CommandOptionParser(commands_dict=VALID_COMMANDS)
  argv = sys.argv

  if len(argv) < 2 or argv[1] not in option_parser.commands_dict:
    # Parse args first, if this is '--help', optparse will print help and exit
    option_parser.parse_args(argv)
    option_parser.error('Invalid command.')

  cmd = option_parser.commands_dict[argv[1]]
  cmd.add_options_func(option_parser)
  options, args = option_parser.parse_args(argv)
  return cmd.run_command_func(argv[1], options, args, option_parser)


if __name__ == '__main__':
  sys.exit(main())
