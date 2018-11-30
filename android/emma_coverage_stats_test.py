#!/usr/bin/env python
# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# pylint: disable=protected-access

import unittest
from xml.etree import ElementTree

import emma_coverage_stats
from pylib.constants import host_paths

with host_paths.SysPath(host_paths.PYMOCK_PATH):
  import mock  # pylint: disable=import-error

EMPTY_COVERAGE_STATS_DICT = {
  'files': {},
  'patch': {
    'incremental': {
      'covered': 0, 'total': 0
    }
  }
}


class _EmmaHtmlParserTest(unittest.TestCase):
  """Tests for _EmmaHtmlParser.

  Uses modified EMMA report HTML that contains only the subset of tags needed
  for test verification.
  """

  def setUp(self):
    self.emma_dir = 'fake/dir/'
    self.parser = emma_coverage_stats._EmmaHtmlParser(self.emma_dir)
    self.simple_html = '<TR><TD CLASS="p">Test HTML</TD></TR>'
    self.index_html = (
      '<HTML>'
        '<BODY>'
          '<TABLE CLASS="hdft" CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
          '<TABLE CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
          '<TABLE CLASS="it" CELLSPACING="0">'
          '</TABLE>'
          '<TABLE CELLSPACING="0" WIDTH="100%">'
            '<TR>'
              '<TH CLASS="f">name</TH>'
              '<TH>class, %</TH>'
              '<TH>method, %</TH>'
              '<TH>block, %</TH>'
              '<TH>line, %</TH>'
            '</TR>'
            '<TR CLASS="o">'
              '<TD><A HREF="_files/0.html"'
              '>org.chromium.chrome.browser</A></TD>'
              '<TD CLASS="h">0%   (0/3)</TD>'
            '</TR>'
            '<TR>'
              '<TD><A HREF="_files/1.html"'
              '>org.chromium.chrome.browser.tabmodel</A></TD>'
              '<TD CLASS="h">0%   (0/8)</TD>'
            '</TR>'
          '</TABLE>'
          '<TABLE CLASS="hdft" CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
        '</BODY>'
      '</HTML>'
    )
    self.package_1_class_list_html = (
      '<HTML>'
        '<BODY>'
          '<TABLE CLASS="hdft" CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
          '<TABLE CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
          '<TABLE CELLSPACING="0" WIDTH="100%">'
            '<TR>'
              '<TH CLASS="f">name</TH>'
              '<TH>class, %</TH>'
              '<TH>method, %</TH>'
              '<TH>block, %</TH>'
              '<TH>line, %</TH>'
            '</TR>'
            '<TR CLASS="o">'
              '<TD><A HREF="1e.html">IntentHelper.java</A></TD>'
              '<TD CLASS="h">0%   (0/3)</TD>'
              '<TD CLASS="h">0%   (0/9)</TD>'
              '<TD CLASS="h">0%   (0/97)</TD>'
              '<TD CLASS="h">0%   (0/26)</TD>'
            '</TR>'
          '</TABLE>'
          '<TABLE CLASS="hdft" CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
        '</BODY>'
      '</HTML>'
    )
    self.package_2_class_list_html = (
      '<HTML>'
        '<BODY>'
          '<TABLE CLASS="hdft" CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
          '<TABLE CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
          '<TABLE CELLSPACING="0" WIDTH="100%">'
            '<TR>'
              '<TH CLASS="f">name</TH>'
              '<TH>class, %</TH>'
              '<TH>method, %</TH>'
              '<TH>block, %</TH>'
              '<TH>line, %</TH>'
            '</TR>'
            '<TR CLASS="o">'
              '<TD><A HREF="1f.html">ContentSetting.java</A></TD>'
              '<TD CLASS="h">0%   (0/1)</TD>'
            '</TR>'
            '<TR>'
              '<TD><A HREF="20.html">DevToolsServer.java</A></TD>'
            '</TR>'
            '<TR CLASS="o">'
              '<TD><A HREF="21.html">FileProviderHelper.java</A></TD>'
            '</TR>'
            '<TR>'
              '<TD><A HREF="22.html">ContextualMenuBar.java</A></TD>'
            '</TR>'
            '<TR CLASS="o">'
              '<TD><A HREF="23.html">AccessibilityUtil.java</A></TD>'
            '</TR>'
            '<TR>'
              '<TD><A HREF="24.html">NavigationPopup.java</A></TD>'
            '</TR>'
          '</TABLE>'
          '<TABLE CLASS="hdft" CELLSPACING="0" WIDTH="100%">'
          '</TABLE>'
        '</BODY>'
      '</HTML>'
    )
    self.partially_covered_tr_html = (
      '<TR CLASS="p">'
        '<TD CLASS="l" TITLE="78% line coverage (7 out of 9)">108</TD>'
        '<TD TITLE="78% line coverage (7 out of 9 instructions)">'
          'if (index &lt; 0 || index = mSelectors.size()) index = 0;</TD>'
      '</TR>'
    )
    self.covered_tr_html = (
      '<TR CLASS="c">'
        '<TD CLASS="l">110</TD>'
        '<TD>        if (mSelectors.get(index) != null) {</TD>'
      '</TR>'
    )
    self.not_executable_tr_html = (
      '<TR>'
        '<TD CLASS="l">109</TD>'
        '<TD> </TD>'
      '</TR>'
    )
    self.tr_with_extra_a_tag = (
      '<TR CLASS="z">'
        '<TD CLASS="l">'
          '<A name="1f">54</A>'
        '</TD>'
        '<TD>            }</TD>'
      '</TR>'
    )

  def testInit(self):
    emma_dir = self.emma_dir
    parser = emma_coverage_stats._EmmaHtmlParser(emma_dir)
    self.assertEqual(parser._base_dir, emma_dir)
    self.assertEqual(parser._emma_files_path, 'fake/dir/_files')
    self.assertEqual(parser._index_path, 'fake/dir/index.html')

  def testFindElements_basic(self):
    read_values = [self.simple_html]
    found, _ = MockOpenForFunction(self.parser._FindElements, read_values,
                                   file_path='fake', xpath_selector='.//TD')
    self.assertIs(type(found), list)
    self.assertIs(type(found[0]), ElementTree.Element)
    self.assertEqual(found[0].text, 'Test HTML')

  def testFindElements_multipleElements(self):
    multiple_trs = self.not_executable_tr_html + self.covered_tr_html
    read_values = ['<div>' + multiple_trs + '</div>']
    found, _ = MockOpenForFunction(self.parser._FindElements, read_values,
                                   file_path='fake', xpath_selector='.//TR')
    self.assertEquals(2, len(found))

  def testFindElements_noMatch(self):
    read_values = [self.simple_html]
    found, _ = MockOpenForFunction(self.parser._FindElements, read_values,
                                   file_path='fake', xpath_selector='.//TR')
    self.assertEqual(found, [])

  def testFindElements_badFilePath(self):
    with self.assertRaises(IOError):
      with mock.patch('os.path.exists', return_value=False):
        self.parser._FindElements('fake', xpath_selector='//tr')

  def testGetPackageNameToEmmaFileDict_basic(self):
    expected_dict = {
      'org.chromium.chrome.browser.AccessibilityUtil.java':
      'fake/dir/_files/23.html',
      'org.chromium.chrome.browser.ContextualMenuBar.java':
      'fake/dir/_files/22.html',
      'org.chromium.chrome.browser.tabmodel.IntentHelper.java':
      'fake/dir/_files/1e.html',
      'org.chromium.chrome.browser.ContentSetting.java':
      'fake/dir/_files/1f.html',
      'org.chromium.chrome.browser.DevToolsServer.java':
      'fake/dir/_files/20.html',
      'org.chromium.chrome.browser.NavigationPopup.java':
      'fake/dir/_files/24.html',
      'org.chromium.chrome.browser.FileProviderHelper.java':
      'fake/dir/_files/21.html'}

    read_values = [self.index_html, self.package_1_class_list_html,
                   self.package_2_class_list_html]
    return_dict, mock_open = MockOpenForFunction(
        self.parser.GetPackageNameToEmmaFileDict, read_values)

    self.assertDictEqual(return_dict, expected_dict)
    self.assertEqual(mock_open.call_count, 3)
    calls = [mock.call('fake/dir/index.html'),
             mock.call('fake/dir/_files/1.html'),
             mock.call('fake/dir/_files/0.html')]
    mock_open.assert_has_calls(calls)

  def testGetPackageNameToEmmaFileDict_noPackageElements(self):
    self.parser._FindElements = mock.Mock(return_value=[])
    return_dict = self.parser.GetPackageNameToEmmaFileDict()
    self.assertDictEqual({}, return_dict)

  def testGetLineCoverage_status_basic(self):
    line_coverage = self.GetLineCoverageWithFakeElements([self.covered_tr_html])
    self.assertEqual(line_coverage[0].covered_status,
                     emma_coverage_stats.COVERED)

  def testGetLineCoverage_status_statusMissing(self):
    line_coverage = self.GetLineCoverageWithFakeElements(
        [self.not_executable_tr_html])
    self.assertEqual(line_coverage[0].covered_status,
                     emma_coverage_stats.NOT_EXECUTABLE)

  def testGetLineCoverage_fractionalCoverage_basic(self):
    line_coverage = self.GetLineCoverageWithFakeElements([self.covered_tr_html])
    self.assertEqual(line_coverage[0].fractional_line_coverage, 1.0)

  def testGetLineCoverage_fractionalCoverage_partial(self):
    line_coverage = self.GetLineCoverageWithFakeElements(
        [self.partially_covered_tr_html])
    self.assertEqual(line_coverage[0].fractional_line_coverage, 0.78)

  def testGetLineCoverage_lineno_basic(self):
    line_coverage = self.GetLineCoverageWithFakeElements([self.covered_tr_html])
    self.assertEqual(line_coverage[0].lineno, 110)

  def testGetLineCoverage_lineno_withAlternativeHtml(self):
    line_coverage = self.GetLineCoverageWithFakeElements(
        [self.tr_with_extra_a_tag])
    self.assertEqual(line_coverage[0].lineno, 54)

  def testGetLineCoverage_source(self):
    self.parser._FindElements = mock.Mock(
        return_value=[ElementTree.fromstring(self.covered_tr_html)])
    line_coverage = self.parser.GetLineCoverage('fake_path')
    self.assertEqual(line_coverage[0].source,
                     '        if (mSelectors.get(index) != null) {')

  def testGetLineCoverage_multipleElements(self):
    line_coverage = self.GetLineCoverageWithFakeElements(
        [self.covered_tr_html, self.partially_covered_tr_html,
         self.tr_with_extra_a_tag])
    self.assertEqual(len(line_coverage), 3)

  def GetLineCoverageWithFakeElements(self, html_elements):
    """Wraps GetLineCoverage so mock HTML can easily be used.

    Args:
      html_elements: List of strings each representing an HTML element.

    Returns:
      A list of LineCoverage objects.
    """
    elements = [ElementTree.fromstring(string) for string in html_elements]
    with mock.patch('emma_coverage_stats._EmmaHtmlParser._FindElements',
                    return_value=elements):
      return self.parser.GetLineCoverage('fake_path')


class _EmmaCoverageStatsTest(unittest.TestCase):
  """Tests for _EmmaCoverageStats."""

  def setUp(self):
    self.good_source_to_emma = {
      '/path/to/1/File1.java': '/emma/1.html',
      '/path/2/File2.java': '/emma/2.html',
      '/path/2/File3.java': '/emma/3.html'
    }
    self.line_coverage = [
        emma_coverage_stats.LineCoverage(
            1, '', emma_coverage_stats.COVERED, 1.0),
        emma_coverage_stats.LineCoverage(
            2, '', emma_coverage_stats.COVERED, 1.0),
        emma_coverage_stats.LineCoverage(
            3, '', emma_coverage_stats.NOT_EXECUTABLE, 1.0),
        emma_coverage_stats.LineCoverage(
            4, '', emma_coverage_stats.NOT_COVERED, 1.0),
        emma_coverage_stats.LineCoverage(
            5, '', emma_coverage_stats.PARTIALLY_COVERED, 0.85),
        emma_coverage_stats.LineCoverage(
            6, '', emma_coverage_stats.PARTIALLY_COVERED, 0.20)
    ]
    self.lines_for_coverage = [1, 3, 5, 6]
    with mock.patch('emma_coverage_stats._EmmaHtmlParser._FindElements',
                    return_value=[]):
      self.simple_coverage = emma_coverage_stats._EmmaCoverageStats(
          'fake_dir', {})

  def testInit(self):
    coverage_stats = self.simple_coverage
    self.assertIsInstance(coverage_stats._emma_parser,
                          emma_coverage_stats._EmmaHtmlParser)
    self.assertIsInstance(coverage_stats._source_to_emma, dict)

  def testNeedsCoverage_withExistingJavaFile(self):
    test_file = '/path/to/file/File.java'
    with mock.patch('os.path.exists', return_value=True):
      self.assertTrue(
          emma_coverage_stats._EmmaCoverageStats.NeedsCoverage(test_file))

  def testNeedsCoverage_withNonJavaFile(self):
    test_file = '/path/to/file/File.c'
    with mock.patch('os.path.exists', return_value=True):
      self.assertFalse(
          emma_coverage_stats._EmmaCoverageStats.NeedsCoverage(test_file))

  def testNeedsCoverage_fileDoesNotExist(self):
    test_file = '/path/to/file/File.java'
    with mock.patch('os.path.exists', return_value=False):
      self.assertFalse(
          emma_coverage_stats._EmmaCoverageStats.NeedsCoverage(test_file))

  def testGetPackageNameFromFile_basic(self):
    test_file_text = """// Test Copyright
    package org.chromium.chrome.browser;
    import android.graphics.RectF;"""
    result_package, _ = MockOpenForFunction(
        emma_coverage_stats._EmmaCoverageStats.GetPackageNameFromFile,
        [test_file_text], file_path='/path/to/file/File.java')
    self.assertEqual(result_package, 'org.chromium.chrome.browser.File.java')

  def testGetPackageNameFromFile_noPackageStatement(self):
    result_package, _ = MockOpenForFunction(
        emma_coverage_stats._EmmaCoverageStats.GetPackageNameFromFile,
        ['not a package statement'], file_path='/path/to/file/File.java')
    self.assertIsNone(result_package)

  def testGetSummaryStatsForLines_basic(self):
    covered, total = self.simple_coverage.GetSummaryStatsForLines(
        self.line_coverage)
    self.assertEqual(covered, 3.05)
    self.assertEqual(total, 5)

  def testGetSourceFileToEmmaFileDict(self):
    package_names = {
      '/path/to/1/File1.java': 'org.fake.one.File1.java',
      '/path/2/File2.java': 'org.fake.File2.java',
      '/path/2/File3.java': 'org.fake.File3.java'
    }
    package_to_emma = {
      'org.fake.one.File1.java': '/emma/1.html',
      'org.fake.File2.java': '/emma/2.html',
      'org.fake.File3.java': '/emma/3.html'
    }
    with mock.patch('os.path.exists', return_value=True):
      coverage_stats = self.simple_coverage
      coverage_stats._emma_parser.GetPackageNameToEmmaFileDict = mock.MagicMock(
          return_value=package_to_emma)
      coverage_stats.GetPackageNameFromFile = lambda x: package_names[x]
      result_dict = coverage_stats._GetSourceFileToEmmaFileDict(
          package_names.keys())
    self.assertDictEqual(result_dict, self.good_source_to_emma)

  def testGetCoverageDictForFile(self):
    line_coverage = self.line_coverage
    self.simple_coverage._emma_parser.GetLineCoverage = lambda x: line_coverage
    self.simple_coverage._source_to_emma = {'/fake/src': 'fake/emma'}
    lines = self.lines_for_coverage
    expected_dict = {
      'absolute': {
        'covered': 3.05,
        'total': 5
      },
      'incremental': {
        'covered': 2.05,
        'total': 3
      },
      'source': [
        {
          'line': line_coverage[0].source,
          'coverage': line_coverage[0].covered_status,
          'changed': True,
          'fractional_coverage': line_coverage[0].fractional_line_coverage,
        },
        {
          'line': line_coverage[1].source,
          'coverage': line_coverage[1].covered_status,
          'changed': False,
          'fractional_coverage': line_coverage[1].fractional_line_coverage,
        },
        {
          'line': line_coverage[2].source,
          'coverage': line_coverage[2].covered_status,
          'changed': True,
          'fractional_coverage': line_coverage[2].fractional_line_coverage,
        },
        {
          'line': line_coverage[3].source,
          'coverage': line_coverage[3].covered_status,
          'changed': False,
          'fractional_coverage': line_coverage[3].fractional_line_coverage,
        },
        {
          'line': line_coverage[4].source,
          'coverage': line_coverage[4].covered_status,
          'changed': True,
          'fractional_coverage': line_coverage[4].fractional_line_coverage,
        },
        {
          'line': line_coverage[5].source,
          'coverage': line_coverage[5].covered_status,
          'changed': True,
          'fractional_coverage': line_coverage[5].fractional_line_coverage,
        }
      ]
    }
    result_dict = self.simple_coverage.GetCoverageDictForFile(
        '/fake/src', lines)
    self.assertDictEqual(result_dict, expected_dict)

  def testGetCoverageDictForFile_emptyCoverage(self):
    expected_dict = {
      'absolute': {'covered': 0, 'total': 0},
      'incremental': {'covered': 0, 'total': 0},
      'source': []
    }
    self.simple_coverage._emma_parser.GetLineCoverage = lambda x: []
    self.simple_coverage._source_to_emma = {'fake_dir': 'fake/emma'}
    result_dict = self.simple_coverage.GetCoverageDictForFile('fake_dir', {})
    self.assertDictEqual(result_dict, expected_dict)

  def testGetCoverageDictForFile_missingCoverage(self):
    self.simple_coverage._source_to_emma = {}
    result_dict = self.simple_coverage.GetCoverageDictForFile('fake_file', {})
    self.assertIsNone(result_dict)

  def testGetCoverageDict_basic(self):
    files_for_coverage = {
      '/path/to/1/File1.java': [1, 3, 4],
      '/path/2/File2.java': [1, 2]
    }
    self.simple_coverage._source_to_emma = {
      '/path/to/1/File1.java': 'emma_1',
      '/path/2/File2.java': 'emma_2'
    }
    coverage_info = {
      'emma_1': [
        emma_coverage_stats.LineCoverage(
            1, '', emma_coverage_stats.COVERED, 1.0),
        emma_coverage_stats.LineCoverage(
            2, '', emma_coverage_stats.PARTIALLY_COVERED, 0.5),
        emma_coverage_stats.LineCoverage(
            3, '', emma_coverage_stats.NOT_EXECUTABLE, 1.0),
        emma_coverage_stats.LineCoverage(
            4, '', emma_coverage_stats.COVERED, 1.0)
      ],
      'emma_2': [
        emma_coverage_stats.LineCoverage(
            1, '', emma_coverage_stats.NOT_COVERED, 1.0),
        emma_coverage_stats.LineCoverage(
            2, '', emma_coverage_stats.COVERED, 1.0)
      ]
    }
    expected_dict = {
      'files': {
        '/path/2/File2.java': {
          'absolute': {'covered': 1, 'total': 2},
          'incremental': {'covered': 1, 'total': 2},
          'source': [{'changed': True, 'coverage': 0,
                      'line': '', 'fractional_coverage': 1.0},
                     {'changed': True, 'coverage': 1,
                      'line': '', 'fractional_coverage': 1.0}]
        },
        '/path/to/1/File1.java': {
          'absolute': {'covered': 2.5, 'total': 3},
          'incremental': {'covered': 2, 'total': 2},
          'source': [{'changed': True, 'coverage': 1,
                      'line': '', 'fractional_coverage': 1.0},
                     {'changed': False, 'coverage': 2,
                      'line': '', 'fractional_coverage': 0.5},
                     {'changed': True, 'coverage': -1,
                      'line': '', 'fractional_coverage': 1.0},
                     {'changed': True, 'coverage': 1,
                      'line': '', 'fractional_coverage': 1.0}]
        }
      },
      'patch': {'incremental': {'covered': 3, 'total': 4}}
    }
    # Return the relevant coverage info for each file.
    self.simple_coverage._emma_parser.GetLineCoverage = (
        lambda x: coverage_info[x])
    result_dict = self.simple_coverage.GetCoverageDict(files_for_coverage)
    self.assertDictEqual(result_dict, expected_dict)

  def testGetCoverageDict_noCoverage(self):
    result_dict = self.simple_coverage.GetCoverageDict({})
    self.assertDictEqual(result_dict, EMPTY_COVERAGE_STATS_DICT)


class EmmaCoverageStatsGenerateCoverageReport(unittest.TestCase):
  """Tests for GenerateCoverageReport."""

  def testGenerateCoverageReport_missingJsonFile(self):
    with self.assertRaises(IOError):
      with mock.patch('os.path.exists', return_value=False):
        emma_coverage_stats.GenerateCoverageReport('', '', '')

  def testGenerateCoverageReport_invalidJsonFile(self):
    with self.assertRaises(ValueError):
      with mock.patch('os.path.exists', return_value=True):
        MockOpenForFunction(emma_coverage_stats.GenerateCoverageReport, [''],
                            line_coverage_file='', out_file_path='',
                            coverage_dir='')


def MockOpenForFunction(func, side_effects, **kwargs):
  """Allows easy mock open and read for callables that open multiple files.

  Will mock the python open function in a way such that each time read() is
  called on an open file, the next element in |side_effects| is returned. This
  makes it easier to test functions that call open() multiple times.

  Args:
    func: The callable to invoke once mock files are setup.
    side_effects: A list of return values for each file to return once read.
      Length of list should be equal to the number calls to open in |func|.
    **kwargs: Keyword arguments to be passed to |func|.

  Returns:
    A tuple containing the return value of |func| and the MagicMock object used
      to mock all calls to open respectively.
  """
  mock_open = mock.mock_open()
  mock_open.side_effect = [mock.mock_open(read_data=side_effect).return_value
                           for side_effect in side_effects]
  with mock.patch('__builtin__.open', mock_open):
    return func(**kwargs), mock_open


if __name__ == '__main__':
  # Suppress logging messages.
  unittest.main(buffer=True)
