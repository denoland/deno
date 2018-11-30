#!/usr/bin/env python
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Generates incremental code coverage reports for Java code in Chromium.

Usage:

  build/android/emma_coverage_stats.py -v --out <output file path> --emma-dir
    <EMMA file directory> --lines-for-coverage-file
    <path to file containing lines for coverage>

  Creates a JSON representation of the overall and file coverage stats and saves
  this information to the specified output file.
"""

import argparse
import collections
import json
import logging
import os
import re
import sys
from xml.etree import ElementTree

import devil_chromium
from devil.utils import run_tests_helper

NOT_EXECUTABLE = -1
NOT_COVERED = 0
COVERED = 1
PARTIALLY_COVERED = 2

# Coverage information about a single line of code.
LineCoverage = collections.namedtuple(
    'LineCoverage',
    ['lineno', 'source', 'covered_status', 'fractional_line_coverage'])


class _EmmaHtmlParser(object):
  """Encapsulates HTML file parsing operations.

  This class contains all operations related to parsing HTML files that were
  produced using the EMMA code coverage tool.

  Example HTML:

  Package links:
    <a href="_files/1.html">org.chromium.chrome</a>
    This is returned by the selector |XPATH_SELECT_PACKAGE_ELEMENTS|.

  Class links:
    <a href="1e.html">DoActivity.java</a>
    This is returned by the selector |XPATH_SELECT_CLASS_ELEMENTS|.

  Line coverage data:
    <tr class="p">
       <td class="l" title="78% line coverage (7 out of 9)">108</td>
       <td title="78% line coverage (7 out of 9 instructions)">
         if (index < 0 || index = mSelectors.size()) index = 0;</td>
    </tr>
    <tr>
       <td class="l">109</td>
       <td> </td>
    </tr>
    <tr class="c">
       <td class="l">110</td>
       <td>        if (mSelectors.get(index) != null) {</td>
    </tr>
    <tr class="z">
       <td class="l">111</td>
       <td>            for (int i = 0; i < mSelectors.size(); i++) {</td>
    </tr>
    Each <tr> element is returned by the selector |XPATH_SELECT_LOC|.

    We can parse this to get:
      1. Line number
      2. Line of source code
      3. Coverage status (c, z, or p)
      4. Fractional coverage value (% out of 100 if PARTIALLY_COVERED)
  """
  # Selector to match all <a> elements within the rows that are in the table
  # that displays all of the different packages.
  _XPATH_SELECT_PACKAGE_ELEMENTS = './/BODY/TABLE[4]/TR/TD/A'

  # Selector to match all <a> elements within the rows that are in the table
  # that displays all of the different classes within a package.
  _XPATH_SELECT_CLASS_ELEMENTS = './/BODY/TABLE[3]/TR/TD/A'

  # Selector to match all <tr> elements within the table containing Java source
  # code in an EMMA HTML file.
  _XPATH_SELECT_LOC = './/BODY/TABLE[4]/TR'

  # Children of HTML elements are represented as a list in ElementTree. These
  # constants represent list indices corresponding to relevant child elements.

  # Child 1 contains percentage covered for a line.
  _ELEMENT_PERCENT_COVERED = 1

  # Child 1 contains the original line of source code.
  _ELEMENT_CONTAINING_SOURCE_CODE = 1

  # Child 0 contains the line number.
  _ELEMENT_CONTAINING_LINENO = 0

  # Maps CSS class names to corresponding coverage constants.
  _CSS_TO_STATUS = {'c': COVERED, 'p': PARTIALLY_COVERED, 'z': NOT_COVERED}

  # UTF-8 no break space.
  _NO_BREAK_SPACE = '\xc2\xa0'

  def __init__(self, emma_file_base_dir):
    """Initializes _EmmaHtmlParser.

    Args:
      emma_file_base_dir: Path to the location where EMMA report files are
        stored. Should be where index.html is stored.
    """
    self._base_dir = emma_file_base_dir
    self._emma_files_path = os.path.join(self._base_dir, '_files')
    self._index_path = os.path.join(self._base_dir, 'index.html')

  def GetLineCoverage(self, emma_file_path):
    """Returns a list of LineCoverage objects for the given EMMA HTML file.

    Args:
      emma_file_path: String representing the path to the EMMA HTML file.

    Returns:
      A list of LineCoverage objects.
    """
    line_tr_elements = self._FindElements(
        emma_file_path, self._XPATH_SELECT_LOC)
    line_coverage = []
    for tr in line_tr_elements:
      # Get the coverage status.
      coverage_status = self._CSS_TO_STATUS.get(tr.get('CLASS'), NOT_EXECUTABLE)
      # Get the fractional coverage value.
      if coverage_status == PARTIALLY_COVERED:
        title_attribute = (tr[self._ELEMENT_PERCENT_COVERED].get('TITLE'))
        # Parse string that contains percent covered: "83% line coverage ...".
        percent_covered = title_attribute.split('%')[0]
        fractional_coverage = int(percent_covered) / 100.0
      else:
        fractional_coverage = 1.0

      # Get the line number.
      lineno_element = tr[self._ELEMENT_CONTAINING_LINENO]
      # Handles oddly formatted HTML (where there is an extra <a> tag).
      lineno = int(lineno_element.text or
                   lineno_element[self._ELEMENT_CONTAINING_LINENO].text)
      # Get the original line of Java source code.
      raw_source = tr[self._ELEMENT_CONTAINING_SOURCE_CODE].text
      utf8_source = raw_source.encode('UTF-8')
      source = utf8_source.replace(self._NO_BREAK_SPACE, ' ')

      line = LineCoverage(lineno, source, coverage_status, fractional_coverage)
      line_coverage.append(line)

    return line_coverage

  def GetPackageNameToEmmaFileDict(self):
    """Returns a dict mapping Java packages to EMMA HTML coverage files.

    Parses the EMMA index.html file to get a list of packages, then parses each
    package HTML file to get a list of classes for that package, and creates
    a dict with this info.

    Returns:
      A dict mapping string representation of Java packages (with class
        names appended) to the corresponding file paths of EMMA HTML files.
    """
    # These <a> elements contain each package name and the path of the file
    # where all classes within said package are listed.
    package_link_elements = self._FindElements(
        self._index_path, self._XPATH_SELECT_PACKAGE_ELEMENTS)
    # Maps file path of package directory (EMMA generated) to package name.
    # Example: emma_dir/f.html: org.chromium.chrome.
    package_links = {
      os.path.join(self._base_dir, link.attrib['HREF']): link.text
      for link in package_link_elements if 'HREF' in link.attrib
    }

    package_to_emma = {}
    for package_emma_file_path, package_name in package_links.iteritems():
      # These <a> elements contain each class name in the current package and
      # the path of the file where the coverage info is stored for each class.
      coverage_file_link_elements = self._FindElements(
          package_emma_file_path, self._XPATH_SELECT_CLASS_ELEMENTS)

      for class_name_element in coverage_file_link_elements:
        emma_coverage_file_path = os.path.join(
            self._emma_files_path, class_name_element.attrib['HREF'])
        full_package_name = '%s.%s' % (package_name, class_name_element.text)
        package_to_emma[full_package_name] = emma_coverage_file_path

    return package_to_emma

  # pylint: disable=no-self-use
  def _FindElements(self, file_path, xpath_selector):
    """Reads a HTML file and performs an XPath match.

    Args:
      file_path: String representing the path to the HTML file.
      xpath_selector: String representing xpath search pattern.

    Returns:
      A list of ElementTree.Elements matching the given XPath selector.
        Returns an empty list if there is no match.
    """
    with open(file_path) as f:
      file_contents = f.read().decode('ISO-8859-1').encode('UTF-8')
      root = ElementTree.fromstring(file_contents)
      return root.findall(xpath_selector)


class _EmmaCoverageStats(object):
  """Computes code coverage stats for Java code using the coverage tool EMMA.

  This class provides an API that allows users to capture absolute code coverage
  and code coverage on a subset of lines for each Java source file. Coverage
  reports are generated in JSON format.
  """
  # Regular expression to get package name from Java package statement.
  RE_PACKAGE_MATCH_GROUP = 'package'
  RE_PACKAGE = re.compile(r'package (?P<%s>[\w.]*);' % RE_PACKAGE_MATCH_GROUP)

  def __init__(self, emma_file_base_dir, files_for_coverage):
    """Initialize _EmmaCoverageStats.

    Args:
      emma_file_base_dir: String representing the path to the base directory
        where EMMA HTML coverage files are stored, i.e. parent of index.html.
      files_for_coverage: A list of Java source code file paths to get EMMA
        coverage for.
    """
    self._emma_parser = _EmmaHtmlParser(emma_file_base_dir)
    self._source_to_emma = self._GetSourceFileToEmmaFileDict(files_for_coverage)

  def GetCoverageDict(self, lines_for_coverage):
    """Returns a dict containing detailed coverage information.

    Gets detailed coverage stats for each file specified in the
    |lines_for_coverage| dict and the total incremental number of lines covered
    and executable for all files in |lines_for_coverage|.

    Args:
      lines_for_coverage: A dict mapping Java source file paths to lists of line
        numbers.

    Returns:
      A dict containing coverage stats for the given dict of files and lines.
        Contains absolute coverage stats for each file, coverage stats for each
        file's lines specified in |lines_for_coverage|, line by line coverage
        for each file, and overall coverage stats for the lines specified in
        |lines_for_coverage|.
    """
    file_coverage = {}
    for file_path, line_numbers in lines_for_coverage.iteritems():
      file_coverage_dict = self.GetCoverageDictForFile(file_path, line_numbers)
      if file_coverage_dict:
        file_coverage[file_path] = file_coverage_dict
      else:
        logging.warning(
            'No code coverage data for %s, skipping.', file_path)

    covered_statuses = [s['incremental'] for s in file_coverage.itervalues()]
    num_covered_lines = sum(s['covered'] for s in covered_statuses)
    num_total_lines = sum(s['total'] for s in covered_statuses)
    return {
      'files': file_coverage,
      'patch': {
        'incremental': {
          'covered': num_covered_lines,
          'total': num_total_lines
        }
      }
    }

  def GetCoverageDictForFile(self, file_path, line_numbers):
    """Returns a dict containing detailed coverage info for the given file.

    Args:
      file_path: The path to the Java source file that we want to create the
        coverage dict for.
      line_numbers: A list of integer line numbers to retrieve additional stats
        for.

    Returns:
      A dict containing absolute, incremental, and line by line coverage for
        a file.
    """
    if file_path not in self._source_to_emma:
      return None
    emma_file = self._source_to_emma[file_path]
    total_line_coverage = self._emma_parser.GetLineCoverage(emma_file)
    incremental_line_coverage = [line for line in total_line_coverage
                                 if line.lineno in line_numbers]
    line_by_line_coverage = [
      {
        'line': line.source,
        'coverage': line.covered_status,
        'changed': line.lineno in line_numbers,
        'fractional_coverage': line.fractional_line_coverage,
      }
      for line in total_line_coverage
    ]
    total_covered_lines, total_lines = (
        self.GetSummaryStatsForLines(total_line_coverage))
    incremental_covered_lines, incremental_total_lines = (
        self.GetSummaryStatsForLines(incremental_line_coverage))

    file_coverage_stats = {
      'absolute': {
        'covered': total_covered_lines,
        'total': total_lines
      },
      'incremental': {
        'covered': incremental_covered_lines,
        'total': incremental_total_lines
      },
      'source': line_by_line_coverage,
    }
    return file_coverage_stats

  # pylint: disable=no-self-use
  def GetSummaryStatsForLines(self, line_coverage):
    """Gets summary stats for a given list of LineCoverage objects.

    Args:
      line_coverage: A list of LineCoverage objects.

    Returns:
      A tuple containing the number of lines that are covered and the total
        number of lines that are executable, respectively
    """
    partially_covered_sum = 0
    covered_status_totals = {COVERED: 0, NOT_COVERED: 0, PARTIALLY_COVERED: 0}
    for line in line_coverage:
      status = line.covered_status
      if status == NOT_EXECUTABLE:
        continue
      covered_status_totals[status] += 1
      if status == PARTIALLY_COVERED:
        partially_covered_sum += line.fractional_line_coverage

    total_covered = covered_status_totals[COVERED] + partially_covered_sum
    total_lines = sum(covered_status_totals.values())
    return total_covered, total_lines

  def _GetSourceFileToEmmaFileDict(self, files):
    """Gets a dict used to correlate Java source files with EMMA HTML files.

    This method gathers the information needed to correlate EMMA HTML
    files with Java source files. EMMA XML and plain text reports do not provide
    line by line coverage data, so HTML reports must be used instead.
    Unfortunately, the HTML files that are created are given garbage names
    (i.e 1.html) so we need to manually correlate EMMA HTML files
    with the original Java source files.

    Args:
      files: A list of file names for which coverage information is desired.

    Returns:
      A dict mapping Java source file paths to EMMA HTML file paths.
    """
    # Maps Java source file paths to package names.
    # Example: /usr/code/file.java -> org.chromium.file.java.
    source_to_package = {}
    for file_path in files:
      package = self.GetPackageNameFromFile(file_path)
      if package:
        source_to_package[file_path] = package
      else:
        logging.warning("Skipping %s because it doesn\'t have a package "
                        "statement.", file_path)

    # Maps package names to EMMA report HTML files.
    # Example: org.chromium.file.java -> out/coverage/1a.html.
    package_to_emma = self._emma_parser.GetPackageNameToEmmaFileDict()
    # Finally, we have a dict mapping Java file paths to EMMA report files.
    # Example: /usr/code/file.java -> out/coverage/1a.html.
    source_to_emma = {source: package_to_emma[package]
                      for source, package in source_to_package.iteritems()
                      if package in package_to_emma}
    return source_to_emma

  @staticmethod
  def NeedsCoverage(file_path):
    """Checks to see if the file needs to be analyzed for code coverage.

    Args:
      file_path: A string representing path to the file.

    Returns:
      True for Java files that exist, False for all others.
    """
    if os.path.splitext(file_path)[1] == '.java' and os.path.exists(file_path):
      return True
    else:
      logging.info('Skipping file %s, cannot compute code coverage.', file_path)
      return False

  @staticmethod
  def GetPackageNameFromFile(file_path):
    """Gets the full package name including the file name for a given file path.

    Args:
      file_path: String representing the path to the Java source file.

    Returns:
      A string representing the full package name with file name appended or
        None if there is no package statement in the file.
    """
    with open(file_path) as f:
      file_content = f.read()
      package_match = re.search(_EmmaCoverageStats.RE_PACKAGE, file_content)
      if package_match:
        package = package_match.group(_EmmaCoverageStats.RE_PACKAGE_MATCH_GROUP)
        file_name = os.path.basename(file_path)
        return '%s.%s' % (package, file_name)
      else:
        return None


def GenerateCoverageReport(line_coverage_file, out_file_path, coverage_dir):
  """Generates a coverage report for a given set of lines.

  Writes the results of the coverage analysis to the file specified by
  |out_file_path|.

  Args:
    line_coverage_file: The path to a file which contains a dict mapping file
      names to lists of line numbers. Example: {file1: [1, 2, 3], ...} means
      that we should compute coverage information on lines 1 - 3 for file1.
    out_file_path: A string representing the location to write the JSON report.
    coverage_dir: A string representing the file path where the EMMA
      HTML coverage files are located (i.e. folder where index.html is located).
  """
  with open(line_coverage_file) as f:
    potential_files_for_coverage = json.load(f)

  files_for_coverage = {f: lines
                        for f, lines in potential_files_for_coverage.iteritems()
                        if _EmmaCoverageStats.NeedsCoverage(f)}

  coverage_results = {}
  if files_for_coverage:
    code_coverage = _EmmaCoverageStats(coverage_dir, files_for_coverage.keys())
    coverage_results = code_coverage.GetCoverageDict(files_for_coverage)
  else:
    logging.info('No Java files requiring coverage were included in %s.',
                 line_coverage_file)

  with open(out_file_path, 'w+') as out_status_file:
    json.dump(coverage_results, out_status_file)


def main():
  argparser = argparse.ArgumentParser()
  argparser.add_argument('--out', required=True, type=str,
                         help='Report output file path.')
  argparser.add_argument('--emma-dir', required=True, type=str,
                         help='EMMA HTML report directory.')
  argparser.add_argument('--lines-for-coverage-file', required=True, type=str,
                         help='File containing a JSON object. Should contain a '
                         'dict mapping file names to lists of line numbers of '
                         'code for which coverage information is desired.')
  argparser.add_argument('-v', '--verbose', action='count',
                         help='Print verbose log information.')
  args = argparser.parse_args()
  run_tests_helper.SetLogLevel(args.verbose)
  devil_chromium.Initialize()
  GenerateCoverageReport(args.lines_for_coverage_file, args.out, args.emma_dir)


if __name__ == '__main__':
  sys.exit(main())
