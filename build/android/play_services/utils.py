# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

'''
Utility functions for all things related to manipulating google play services
related files.
'''

import argparse
import filecmp
import json
import os
import re


_XML_VERSION_NUMBER_PATTERN = re.compile(
    r'<integer name="google_play_services_version">(\d+)<\/integer>')


class DefaultsRawHelpFormatter(argparse.ArgumentDefaultsHelpFormatter,
                               argparse.RawDescriptionHelpFormatter):
  '''
  Combines the features of RawDescriptionHelpFormatter and
  ArgumentDefaultsHelpFormatter, providing defaults for the arguments and raw
  text for the description.
  '''
  pass


class ConfigParser(object):
  '''Reads and writes the configuration files for play services related scripts

  The configuration files are JSON files. Here is the data they are expected
  to contain:

   -  version_number
      Number. Mirrors @integer/google_play_services_version from the library.
      Example: 815000

   -  sdk_version
      Version of the Play Services SDK to retrieve, when preprocessing the
      library from a maven/gradle repository.
      Example: "8.1.0"

   -  clients
      List of strings. Name of the clients (or play services modules) to
      include when preprocessing the library.
      Example: ["play-services-base", "play-services-cast"]

   -  version_xml_path
      String. Path to the version.xml string describing the current version.
      Should be relative to the library base directory
      Example: "res/values/version.xml"

   -  locale_whitelist
      List of strings. List of locales to keep from the resources. Can be
      obtained by generating an android build and looking at the content of
      `out/Debug/gen/chrome/java/res`; or looking at the android section in
      `//chrome/app/generated_resources.grd`
      Example: ["am", "ar", "bg", "ca", "cs"]

   - resource_whitelist
     List of strings. List of resource files to explicitely keep in the final
     output. Use it to keep drawables for example, as we currently remove them
     all.
     Example: ["play-services-base/res/drawables/foobar.xml"]
  '''
  _VERSION_NUMBER_KEY = 'version_number'

  def __init__(self, path):
    self.path = path
    self._data = {}

    with open(path, 'r') as stream:
      self._data = json.load(stream)

  @property
  def version_number(self):
    return self._data.get(self._VERSION_NUMBER_KEY)

  @property
  def sdk_version(self):
    return self._data.get('sdk_version')

  @property
  def clients(self):
    return self._data.get('clients') or []

  @property
  def version_xml_path(self):
    return self._data.get('version_xml_path')

  @property
  def locale_whitelist(self):
    return self._data.get('locale_whitelist') or []

  @property
  def resource_whitelist(self):
    return self._data.get('resource_whitelist') or []

  def UpdateVersionNumber(self, new_version_number):
    '''Updates the version number and saves it in the configuration file. '''

    with open(self.path, 'w') as stream:
      self._data[self._VERSION_NUMBER_KEY] = new_version_number
      stream.write(DumpTrimmedJson(self._data))


def DumpTrimmedJson(json_data):
  '''
  Default formatting when dumping json to string has trailing spaces and lacks
  a new line at the end. This function fixes that.
  '''

  out = json.dumps(json_data, sort_keys=True, indent=2)
  out = out.replace(' ' + os.linesep, os.linesep)
  return out + os.linesep


def FileEquals(expected_file, actual_file):
  '''
  Returns whether the two files are equal. Returns False if any of the files
  doesn't exist.
  '''

  if not os.path.isfile(actual_file) or not os.path.isfile(expected_file):
    return False
  return filecmp.cmp(expected_file, actual_file)


def GetVersionNumberFromLibraryResources(version_xml):
  '''
  Extracts a Google Play services version number from its version.xml file.
  '''

  with open(version_xml, 'r') as version_file:
    version_file_content = version_file.read()

  match = _XML_VERSION_NUMBER_PATTERN.search(version_file_content)
  if not match:
    raise AttributeError('A value for google_play_services_version was not '
                         'found in ' + version_xml)
  return int(match.group(1))
