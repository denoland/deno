#!/usr/bin/env python
#
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import os
import subprocess
import sys

from util import build_utils


def main():
  # The point of this wrapper is to use AtomicOutput so that output timestamps
  # are not updated when outputs are unchanged.
  ijar_bin, in_jar, out_jar = sys.argv[1:]
  with build_utils.AtomicOutput(out_jar) as f:
    subprocess.check_call([ijar_bin, in_jar, f.name])


if __name__ == '__main__':
  main()
