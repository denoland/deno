# Copyright (c) 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Presubmit script for //build.

See http://dev.chromium.org/developers/how-tos/depottools/presubmit-scripts
for more details about the presubmit API built into depot_tools.
"""

def PostUploadHook(cl, change, output_api):
  """git cl upload will call this hook after the issue is created/modified.

  This hook modifies the CL description in order to run extra tests.
  """

  def affects_gn_checker(f):
    return 'check_gn_headers' in f.LocalPath()
  if not change.AffectedFiles(file_filter=affects_gn_checker):
    return []
  return output_api.EnsureCQIncludeTrybotsAreAdded(
    cl,
    [
      'luci.chromium.try:linux_chromium_dbg_ng',
    ],
    'Automatically added tests to run on CQ.')
