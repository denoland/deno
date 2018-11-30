# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# This script shows cpu count to specify capacity of action pool.

import multiprocessing
import sys

def main():
  try:
    cpu_count = multiprocessing.cpu_count()
  except:
    cpu_count = 1

  print cpu_count
  return 0


if __name__ == '__main__':
  sys.exit(main())
