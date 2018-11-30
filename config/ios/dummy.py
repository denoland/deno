# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Empty script that does nothing and return success error code.

This script is used by some gn targets that pretend creating some output
but instead depend on another target creating the output indirectly (in
general this output is a directory that is used as input by a bundle_data
target).

It ignores all parameters and terminate with a success error code. It
does the same thing as the unix command "true", but gn can only invoke
python scripts.
"""
