#!/usr/bin/env python
#
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import collections
import contextlib
import json
import logging
import tempfile
import os
import sys
import urllib


CURRENT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = os.path.abspath(os.path.join(
    CURRENT_DIR, '..', '..', '..', '..', '..'))

sys.path.append(os.path.join(BASE_DIR, 'build', 'android'))
from pylib.results.presentation import standard_gtest_merge
from pylib.utils import google_storage_helper  # pylint: disable=import-error

sys.path.append(os.path.join(BASE_DIR, 'third_party'))
import jinja2  # pylint: disable=import-error
JINJA_ENVIRONMENT = jinja2.Environment(
    loader=jinja2.FileSystemLoader(os.path.dirname(__file__)),
    autoescape=True)


def cell(data, html_class='center'):
  """Formats table cell data for processing in jinja template."""
  return {
    'data': data,
    'class': html_class,
  }


def pre_cell(data, html_class='center'):
  """Formats table <pre> cell data for processing in jinja template."""
  return {
    'cell_type': 'pre',
    'data': data,
    'class': html_class,
  }


class LinkTarget(object):
  # Opens the linked document in a new window or tab.
  NEW_TAB = '_blank'
  # Opens the linked document in the same frame as it was clicked.
  CURRENT_TAB = '_self'


def link(data, href, target=LinkTarget.CURRENT_TAB):
  """Formats <a> tag data for processing in jinja template.

  Args:
    data: String link appears as on HTML page.
    href: URL where link goes.
    target: Where link should be opened (e.g. current tab or new tab).
  """
  return {
    'data': data,
    'href': href,
    'target': target,
  }


def links_cell(links, html_class='center', rowspan=None):
  """Formats table cell with links for processing in jinja template.

  Args:
    links: List of link dictionaries. Use |link| function to generate them.
    html_class: Class for table cell.
    rowspan: Rowspan HTML attribute.
  """
  return {
    'cell_type': 'links',
    'class': html_class,
    'links': links,
    'rowspan': rowspan,
  }


def action_cell(action, data, html_class):
  """Formats table cell with javascript actions.

  Args:
    action: Javscript action.
    data: Data in cell.
    class: Class for table cell.
  """
  return {
    'cell_type': 'action',
    'action': action,
    'data': data,
    'class': html_class,
  }


def flakiness_dashbord_link(test_name, suite_name):
  url_args = urllib.urlencode([
      ('testType', suite_name),
      ('tests', test_name)])
  return ('https://test-results.appspot.com/'
         'dashboards/flakiness_dashboard.html#%s' % url_args)


def logs_cell(result, test_name, suite_name):
  """Formats result logs data for processing in jinja template."""
  link_list = []
  result_link_dict = result.get('links', {})
  result_link_dict['flakiness'] = flakiness_dashbord_link(
      test_name, suite_name)
  for name, href in sorted(result_link_dict.items()):
    link_list.append(link(
        data=name,
        href=href,
        target=LinkTarget.NEW_TAB))
  if link_list:
    return links_cell(link_list)
  else:
    return cell('(no logs)')


def code_search(test, cs_base_url):
  """Returns URL for test on codesearch."""
  search = test.replace('#', '.')
  return '%s/?q=%s&type=cs' % (cs_base_url, search)


def status_class(status):
  """Returns HTML class for test status."""
  if not status:
    return 'failure unknown'
  status = status.lower()
  if status not in ('success', 'skipped'):
    return 'failure %s' % status
  return status


def create_test_table(results_dict, cs_base_url, suite_name):
  """Format test data for injecting into HTML table."""

  header_row = [
    cell(data='test_name', html_class='text'),
    cell(data='status', html_class='flaky'),
    cell(data='elapsed_time_ms', html_class='number'),
    cell(data='logs', html_class='text'),
    cell(data='output_snippet', html_class='text'),
  ]

  test_row_blocks = []
  for test_name, test_results in results_dict.iteritems():
    test_runs = []
    for index, result in enumerate(test_results):
      if index == 0:
        test_run = [links_cell(
            links=[
                link(href=code_search(test_name, cs_base_url),
                     target=LinkTarget.NEW_TAB,
                     data=test_name)],
            rowspan=len(test_results),
            html_class='left %s' % test_name
        )]                                          # test_name
      else:
        test_run = []

      test_run.extend([
          cell(data=result['status'] or 'UNKNOWN',
                                                    # status
               html_class=('center %s' %
                  status_class(result['status']))),
          cell(data=result['elapsed_time_ms']),     # elapsed_time_ms
          logs_cell(result, test_name, suite_name), # logs
          pre_cell(data=result['output_snippet'],   # output_snippet
                   html_class='left'),
      ])
      test_runs.append(test_run)
    test_row_blocks.append(test_runs)
  return header_row, test_row_blocks


def create_suite_table(results_dict):
  """Format test suite data for injecting into HTML table."""

  SUCCESS_COUNT_INDEX = 1
  FAIL_COUNT_INDEX = 2
  ALL_COUNT_INDEX = 3
  TIME_INDEX = 4

  header_row = [
    cell(data='suite_name', html_class='text'),
    cell(data='number_success_tests', html_class='number'),
    cell(data='number_fail_tests', html_class='number'),
    cell(data='all_tests', html_class='number'),
    cell(data='elapsed_time_ms', html_class='number'),
  ]

  footer_row = [
    action_cell(
          'showTestsOfOneSuiteOnlyWithNewState("TOTAL")',
          'TOTAL',
          'center'
        ),         # TOTAL
    cell(data=0),  # number_success_tests
    cell(data=0),  # number_fail_tests
    cell(data=0),  # all_tests
    cell(data=0),  # elapsed_time_ms
  ]

  suite_row_dict = {}
  for test_name, test_results in results_dict.iteritems():
    # TODO(mikecase): This logic doesn't work if there are multiple test runs.
    # That is, if 'per_iteration_data' has multiple entries.
    # Since we only care about the result of the last test run.
    result = test_results[-1]

    suite_name = (test_name.split('#')[0] if '#' in test_name
                  else test_name.split('.')[0])
    if suite_name in suite_row_dict:
      suite_row = suite_row_dict[suite_name]
    else:
      suite_row = [
        action_cell(
          'showTestsOfOneSuiteOnlyWithNewState("%s")' % suite_name,
          suite_name,
          'left'
        ),             # suite_name
        cell(data=0),  # number_success_tests
        cell(data=0),  # number_fail_tests
        cell(data=0),  # all_tests
        cell(data=0),  # elapsed_time_ms
      ]

    suite_row_dict[suite_name] = suite_row

    suite_row[ALL_COUNT_INDEX]['data'] += 1
    footer_row[ALL_COUNT_INDEX]['data'] += 1

    if result['status'] == 'SUCCESS':
      suite_row[SUCCESS_COUNT_INDEX]['data'] += 1
      footer_row[SUCCESS_COUNT_INDEX]['data'] += 1
    elif result['status'] != 'SKIPPED':
      suite_row[FAIL_COUNT_INDEX]['data'] += 1
      footer_row[FAIL_COUNT_INDEX]['data'] += 1

    suite_row[TIME_INDEX]['data'] += result['elapsed_time_ms']
    footer_row[TIME_INDEX]['data'] += result['elapsed_time_ms']

  for suite in suite_row_dict.values():
    if suite[FAIL_COUNT_INDEX]['data'] > 0:
      suite[FAIL_COUNT_INDEX]['class'] += ' failure'
    else:
      suite[FAIL_COUNT_INDEX]['class'] += ' success'

  if footer_row[FAIL_COUNT_INDEX]['data'] > 0:
    footer_row[FAIL_COUNT_INDEX]['class'] += ' failure'
  else:
    footer_row[FAIL_COUNT_INDEX]['class'] += ' success'

  return (header_row,
          [[suite_row] for suite_row in suite_row_dict.values()],
          footer_row)


def feedback_url(result_details_link):
  # pylint: disable=redefined-variable-type
  url_args = [
      ('labels', 'Pri-2,Type-Bug,Restrict-View-Google'),
      ('summary', 'Result Details Feedback:'),
      ('components', 'Test>Android'),
  ]
  if result_details_link:
    url_args.append(('comment', 'Please check out: %s' % result_details_link))
  url_args = urllib.urlencode(url_args)
  # pylint: enable=redefined-variable-type
  return 'https://bugs.chromium.org/p/chromium/issues/entry?%s' % url_args


def results_to_html(results_dict, cs_base_url, bucket, test_name,
                    builder_name, build_number, local_output):
  """Convert list of test results into html format.

  Args:
    local_output: Whether this results file is uploaded to Google Storage or
        just a local file.
  """
  test_rows_header, test_rows = create_test_table(
      results_dict, cs_base_url, test_name)
  suite_rows_header, suite_rows, suite_row_footer = create_suite_table(
      results_dict)

  suite_table_values = {
    'table_id': 'suite-table',
    'table_headers': suite_rows_header,
    'table_row_blocks': suite_rows,
    'table_footer': suite_row_footer,
  }

  test_table_values = {
    'table_id': 'test-table',
    'table_headers': test_rows_header,
    'table_row_blocks': test_rows,
  }

  main_template = JINJA_ENVIRONMENT.get_template(
      os.path.join('template', 'main.html'))

  if local_output:
    html_render = main_template.render(  #  pylint: disable=no-member
        {
          'tb_values': [suite_table_values, test_table_values],
          'feedback_url': feedback_url(None),
        })
    return (html_render, None, None)
  else:
    dest = google_storage_helper.unique_name(
        '%s_%s_%s' % (test_name, builder_name, build_number))
    result_details_link = google_storage_helper.get_url_link(
        dest, '%s/html' % bucket)
    html_render = main_template.render(  #  pylint: disable=no-member
        {
          'tb_values': [suite_table_values, test_table_values],
          'feedback_url': feedback_url(result_details_link),
        })
    return (html_render, dest, result_details_link)


def result_details(json_path, test_name, cs_base_url, bucket=None,
                   builder_name=None, build_number=None, local_output=False):
  """Get result details from json path and then convert results to html.

  Args:
    local_output: Whether this results file is uploaded to Google Storage or
        just a local file.
  """

  with open(json_path) as json_file:
    json_object = json.loads(json_file.read())

  if not 'per_iteration_data' in json_object:
    return 'Error: json file missing per_iteration_data.'

  results_dict = collections.defaultdict(list)
  for testsuite_run in json_object['per_iteration_data']:
    for test, test_runs in testsuite_run.iteritems():
      results_dict[test].extend(test_runs)
  return results_to_html(results_dict, cs_base_url, bucket, test_name,
                         builder_name, build_number, local_output)


def upload_to_google_bucket(html, bucket, dest):
  with tempfile.NamedTemporaryFile(suffix='.html') as temp_file:
    temp_file.write(html)
    temp_file.flush()
    return google_storage_helper.upload(
        name=dest,
        filepath=temp_file.name,
        bucket='%s/html' % bucket,
        content_type='text/html',
        authenticated_link=True)


def ui_screenshot_set(json_path):
  with open(json_path) as json_file:
    json_object = json.loads(json_file.read())
  if not 'per_iteration_data' in json_object:
    # This will be reported as an error by result_details, no need to duplicate.
    return None
  ui_screenshots = []
  # pylint: disable=too-many-nested-blocks
  for testsuite_run in json_object['per_iteration_data']:
    for _, test_runs in testsuite_run.iteritems():
      for test_run in test_runs:
        if 'ui screenshot' in test_run['links']:
          screenshot_link = test_run['links']['ui screenshot']
          if screenshot_link.startswith('file:'):
            with contextlib.closing(urllib.urlopen(screenshot_link)) as f:
              test_screenshots = json.load(f)
          else:
            # Assume anything that isn't a file link is a google storage link
            screenshot_string = google_storage_helper.read_from_link(
                screenshot_link)
            if not screenshot_string:
              logging.error('Bad screenshot link %s', screenshot_link)
              continue
            test_screenshots = json.loads(
                screenshot_string)
          ui_screenshots.extend(test_screenshots)
  # pylint: enable=too-many-nested-blocks

  if ui_screenshots:
    return json.dumps(ui_screenshots)
  return None


def upload_screenshot_set(json_path, test_name, bucket, builder_name,
                          build_number):
  screenshot_set = ui_screenshot_set(json_path)
  if not screenshot_set:
    return None
  dest = google_storage_helper.unique_name(
    'screenshots_%s_%s_%s' % (test_name, builder_name, build_number),
    suffix='.json')
  with tempfile.NamedTemporaryFile(suffix='.json') as temp_file:
    temp_file.write(screenshot_set)
    temp_file.flush()
    return google_storage_helper.upload(
        name=dest,
        filepath=temp_file.name,
        bucket='%s/json' % bucket,
        content_type='application/json',
        authenticated_link=True)


def main():
  parser = argparse.ArgumentParser()
  parser.add_argument('--json-file', help='Path of json file.')
  parser.add_argument('--cs-base-url', help='Base url for code search.',
                      default='http://cs.chromium.org')
  parser.add_argument('--bucket', help='Google storage bucket.', required=True)
  parser.add_argument('--builder-name', help='Builder name.')
  parser.add_argument('--build-number', help='Build number.')
  parser.add_argument('--test-name', help='The name of the test.',
                      required=True)
  parser.add_argument(
      '-o', '--output-json',
      help='(Swarming Merge Script API) '
           'Output JSON file to create.')
  parser.add_argument(
      '--build-properties',
      help='(Swarming Merge Script API) '
           'Build property JSON file provided by recipes.')
  parser.add_argument(
      '--summary-json',
      help='(Swarming Merge Script API) '
           'Summary of shard state running on swarming. '
           '(Output of the swarming.py collect '
           '--task-summary-json=XXX command.)')
  parser.add_argument(
      '--task-output-dir',
      help='(Swarming Merge Script API) '
           'Directory containing all swarming task results.')
  parser.add_argument(
      'positional', nargs='*',
      help='output.json from shards.')

  args = parser.parse_args()

  if ((args.build_properties is None) ==
         (args.build_number is None or args.builder_name is None)):
    raise parser.error('Exactly one of build_perperties or '
                       '(build_number or builder_name) should be given.')

  if (args.build_number is None) != (args.builder_name is None):
    raise parser.error('args.build_number and args.builder_name '
                       'has to be be given together'
                       'or not given at all.')

  if len(args.positional) == 0 and args.json_file is None:
    if args.output_json:
        with open(args.output_json, 'w') as f:
          json.dump({}, f)
    return
  elif len(args.positional) != 0 and args.json_file:
    raise parser.error('Exactly one of args.positional and '
                       'args.json_file should be given.')

  if args.build_properties:
    build_properties = json.loads(args.build_properties)
    if ((not 'buildnumber' in build_properties) or
        (not 'buildername' in build_properties)):
      raise parser.error('Build number/builder name not specified.')
    build_number = build_properties['buildnumber']
    builder_name = build_properties['buildername']
  elif args.build_number and args.builder_name:
    build_number = args.build_number
    builder_name = args.builder_name

  if args.positional:
    if len(args.positional) == 1:
      json_file = args.positional[0]
    else:
      if args.output_json and args.summary_json:
        standard_gtest_merge.standard_gtest_merge(
            args.output_json, args.summary_json, args.positional)
        json_file = args.output_json
      elif not args.output_json:
        raise Exception('output_json required by merge API is missing.')
      else:
        raise Exception('summary_json required by merge API is missing.')
  elif args.json_file:
    json_file = args.json_file

  if not os.path.exists(json_file):
    raise IOError('--json-file %s not found.' % json_file)

  # Link to result details presentation page is a part of the page.
  result_html_string, dest, result_details_link = result_details(
      json_file, args.test_name, args.cs_base_url, args.bucket,
      builder_name, build_number)

  result_details_link_2 = upload_to_google_bucket(
      result_html_string.encode('UTF-8'),
      args.bucket, dest)
  assert result_details_link == result_details_link_2, (
      'Result details link do not match. The link returned by get_url_link'
      ' should be the same as that returned by upload.')

  ui_screenshot_set_link = upload_screenshot_set(json_file, args.test_name,
      args.bucket, builder_name, build_number)

  if ui_screenshot_set_link:
    ui_catalog_url = 'https://chrome-ui-catalog.appspot.com/'
    ui_catalog_query = urllib.urlencode(
        {'screenshot_source': ui_screenshot_set_link})
    ui_screenshot_link = '%s?%s' % (ui_catalog_url, ui_catalog_query)

  if args.output_json:
    with open(json_file) as original_json_file:
      json_object = json.load(original_json_file)
      json_object['links'] = {
          'result_details (logcats, flakiness links)': result_details_link
      }

      if ui_screenshot_set_link:
        json_object['links']['ui screenshots'] = ui_screenshot_link

      with open(args.output_json, 'w') as f:
        json.dump(json_object, f)
  else:
    print 'Result Details: %s' % result_details_link

    if ui_screenshot_set_link:
      print 'UI Screenshots %s' % ui_screenshot_link


if __name__ == '__main__':
  sys.exit(main())
