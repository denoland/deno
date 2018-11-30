# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
import re


_CMDLINE_NAME_SEGMENT_RE = re.compile(
    r' with(?:out)? \{[^\}]*\}')


def ParseFilterFile(input_lines):
  """Converts test filter file contents into --gtest_filter argument.

  See //testing/buildbot/filters/README.md for description of the
  syntax that |input_lines| are expected to follow.

  See
  https://github.com/google/googletest/blob/master/googletest/docs/AdvancedGuide.md#running-a-subset-of-the-tests
  for description of the syntax that --gtest_filter argument should follow.

  Args:
    input_lines: An iterable (e.g. a list or a file) containing input lines.
  Returns:
    a string suitable for feeding as an argument of --gtest_filter parameter.
  """
  # Strip comments and whitespace from each line and filter non-empty lines.
  stripped_lines = (l.split('#', 1)[0].strip() for l in input_lines)
  filter_lines = [l for l in stripped_lines if l]

  # Split the tests into positive and negative patterns (gtest treats
  # every pattern after the first '-' sign as an exclusion).
  positive_patterns = ':'.join(l for l in filter_lines if l[0] != '-')
  negative_patterns = ':'.join(l[1:] for l in filter_lines if l[0] == '-')
  if negative_patterns:
    negative_patterns = '-' + negative_patterns

  # Join the filter lines into one, big --gtest_filter argument.
  return positive_patterns + negative_patterns


def AddFilterOptions(parser):
  """Adds filter command-line options to the provided parser.

  Args:
    parser: an argparse.ArgumentParser instance.
  """
  filter_group = parser.add_mutually_exclusive_group()
  filter_group.add_argument(
      '-f', '--test-filter', '--gtest_filter', '--gtest-filter',
      dest='test_filter',
      help='googletest-style filter string.',
      default=os.environ.get('GTEST_FILTER'))
  filter_group.add_argument(
      # Deprecated argument.
      '--gtest-filter-file',
      # New argument.
      '--test-launcher-filter-file',
      dest='test_filter_file', type=os.path.realpath,
      help='Path to file that contains googletest-style filter strings. '
           'See also //testing/buildbot/filters/README.md.')


def InitializeFilterFromArgs(args):
  """Returns a filter string from the command-line option values.

  Args:
    args: an argparse.Namespace instance resulting from a using parser
      to which the filter options above were added.
  """
  parsed_filter = None
  if args.test_filter:
    parsed_filter = _CMDLINE_NAME_SEGMENT_RE.sub(
        '', args.test_filter.replace('#', '.'))
  elif args.test_filter_file:
    with open(args.test_filter_file, 'r') as f:
      parsed_filter = ParseFilterFile(f)

  return parsed_filter
