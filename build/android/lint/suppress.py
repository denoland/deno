#!/usr/bin/env python
#
# Copyright (c) 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Add all generated lint_result.xml files to suppressions.xml"""

# pylint: disable=no-member


import argparse
import collections
import os
import re
import sys
from xml.dom import minidom

_BUILD_ANDROID_DIR = os.path.join(os.path.dirname(__file__), '..')
sys.path.append(_BUILD_ANDROID_DIR)

from pylib.constants import host_paths

_TMP_DIR_RE = re.compile(r'^/tmp/.*/(SRC_ROOT[0-9]+|PRODUCT_DIR)/')
_THIS_FILE = os.path.abspath(__file__)
_DEFAULT_CONFIG_PATH = os.path.join(os.path.dirname(_THIS_FILE),
                                    'suppressions.xml')
_DOC = (
    '\nSTOP! It looks like you want to suppress some lint errors:\n'
    '- Have you tried identifing the offending patch?\n'
    '  Ask the author for a fix and/or revert the patch.\n'
    '- It is preferred to add suppressions in the code instead of\n'
    '  sweeping it under the rug here. See:\n\n'
    '    http://developer.android.com/tools/debugging/improving-w-lint.html\n'
    '\n'
    'Still reading?\n'
    '- You can edit this file manually to suppress an issue\n'
    '  globally if it is not applicable to the project.\n'
    '- You can also automatically add issues found so for in the\n'
    '  build process by running:\n\n'
    '    ' + os.path.relpath(_THIS_FILE, host_paths.DIR_SOURCE_ROOT) + '\n\n'
    '  which will generate this file (Comments are not preserved).\n'
    '  Note: PRODUCT_DIR will be substituted at run-time with actual\n'
    '  directory path (e.g. out/Debug)\n'
)


_Issue = collections.namedtuple('Issue', ['severity', 'paths', 'regexps'])


def _ParseConfigFile(config_path):
  print 'Parsing %s' % config_path
  issues_dict = {}
  dom = minidom.parse(config_path)
  for issue in dom.getElementsByTagName('issue'):
    issue_id = issue.attributes['id'].value
    severity = issue.getAttribute('severity')

    path_elements = (
        p.attributes.get('path')
        for p in issue.getElementsByTagName('ignore'))
    paths = set(p.value for p in path_elements if p)

    regexp_elements = (
        p.attributes.get('regexp')
        for p in issue.getElementsByTagName('ignore'))
    regexps = set(r.value for r in regexp_elements if r)

    issues_dict[issue_id] = _Issue(severity, paths, regexps)
  return issues_dict


def _ParseAndMergeResultFile(result_path, issues_dict):
  print 'Parsing and merging %s' % result_path
  dom = minidom.parse(result_path)
  for issue in dom.getElementsByTagName('issue'):
    issue_id = issue.attributes['id'].value
    severity = issue.attributes['severity'].value
    path = issue.getElementsByTagName('location')[0].attributes['file'].value
    # Strip temporary file path.
    path = re.sub(_TMP_DIR_RE, '', path)
    # Escape Java inner class name separator and suppress with regex instead
    # of path. Doesn't use re.escape() as it is a bit too aggressive and
    # escapes '_', causing trouble with PRODUCT_DIR.
    regexp = path.replace('$', r'\$')
    if issue_id not in issues_dict:
      issues_dict[issue_id] = _Issue(severity, set(), set())
    issues_dict[issue_id].regexps.add(regexp)


def _WriteConfigFile(config_path, issues_dict):
  new_dom = minidom.getDOMImplementation().createDocument(None, 'lint', None)
  top_element = new_dom.documentElement
  top_element.appendChild(new_dom.createComment(_DOC))
  for issue_id, issue in sorted(issues_dict.iteritems(), key=lambda i: i[0]):
    issue_element = new_dom.createElement('issue')
    issue_element.attributes['id'] = issue_id
    if issue.severity:
      issue_element.attributes['severity'] = issue.severity
    if issue.severity == 'ignore':
      print 'Warning: [%s] is suppressed globally.' % issue_id
    else:
      for path in sorted(issue.paths):
        ignore_element = new_dom.createElement('ignore')
        ignore_element.attributes['path'] = path
        issue_element.appendChild(ignore_element)
      for regexp in sorted(issue.regexps):
        ignore_element = new_dom.createElement('ignore')
        ignore_element.attributes['regexp'] = regexp
        issue_element.appendChild(ignore_element)
    top_element.appendChild(issue_element)

  with open(config_path, 'w') as f:
    f.write(new_dom.toprettyxml(indent='  ', encoding='utf-8'))
  print 'Updated %s' % config_path


def _Suppress(config_path, result_path):
  issues_dict = _ParseConfigFile(config_path)
  _ParseAndMergeResultFile(result_path, issues_dict)
  _WriteConfigFile(config_path, issues_dict)


def main():
  parser = argparse.ArgumentParser(description=__doc__)
  parser.add_argument('--config',
                      help='Path to suppression.xml config file',
                      default=_DEFAULT_CONFIG_PATH)
  parser.add_argument('result_path',
                      help='Lint results xml file',
                      metavar='RESULT_FILE')
  args = parser.parse_args()

  _Suppress(args.config, args.result_path)


if __name__ == '__main__':
  main()
