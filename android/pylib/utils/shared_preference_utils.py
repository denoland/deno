# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Utility functions for modifying an app's settings file using JSON."""

import json
import logging


def UnicodeToStr(data):
  """Recursively converts any Unicode to Python strings.

  Args:
    data: The data to be converted.

  Return:
    A copy of the given data, but with instances of Unicode converted to Python
    strings.
  """
  if isinstance(data, dict):
    return {UnicodeToStr(key): UnicodeToStr(value)
            for key, value in data.iteritems()}
  elif isinstance(data, list):
    return [UnicodeToStr(element) for element in data]
  elif isinstance(data, unicode):
    return data.encode('utf-8')
  return data


def ExtractSettingsFromJson(filepath):
  """Extracts the settings data from the given JSON file.

  Args:
    filepath: The path to the JSON file to read.

  Return:
    The data read from the JSON file with strings converted to Python strings.
  """
  # json.load() loads strings as unicode, which causes issues when trying
  # to edit string values in preference files, so convert to Python strings
  with open(filepath) as prefs_file:
    return UnicodeToStr(json.load(prefs_file))


def ApplySharedPreferenceSetting(shared_pref, setting):
  """Applies the given app settings to the given device.

  Modifies an installed app's settings by modifying its shared preference
  settings file. Provided settings data must be a settings dictionary,
  which are in the following format:
  {
    "package": "com.example.package",
    "filename": "AppSettingsFile.xml",
    "supports_encrypted_path": true,
    "set": {
      "SomeBoolToSet": true,
      "SomeStringToSet": "StringValue",
    },
    "remove": [
      "list",
      "of",
      "keys",
      "to",
      "remove",
    ]
  }

  Example JSON files that can be read with ExtractSettingsFromJson and passed to
  this function are in //chrome/android/shared_preference_files/test/.

  Args:
    shared_pref: The devil SharedPrefs object for the device the settings will
        be applied to.
    setting: A settings dictionary to apply.
  """
  shared_pref.Load()
  for key in setting.get('remove', []):
    try:
      shared_pref.Remove(key)
    except KeyError:
      logging.warning("Attempted to remove non-existent key %s", key)
  for key, value in setting.get('set', {}).iteritems():
    if isinstance(value, bool):
      shared_pref.SetBoolean(key, value)
    elif isinstance(value, basestring):
      shared_pref.SetString(key, value)
    elif isinstance(value, long) or isinstance(value, int):
      shared_pref.SetLong(key, value)
    elif isinstance(value, list):
      shared_pref.SetStringSet(key, value)
    else:
      raise ValueError("Given invalid value type %s for key %s" % (
          str(type(value)), key))
  shared_pref.Commit()
