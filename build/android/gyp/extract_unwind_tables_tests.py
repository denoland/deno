#!/usr/bin/env python
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Tests for extract_unwind_tables.py

This test suite contains various tests for extracting CFI tables from breakpad
symbol files.
"""

import optparse
import os
import struct
import sys
import tempfile
import unittest

import extract_unwind_tables

sys.path.append(os.path.join(os.path.dirname(__file__), "gyp"))
from util import build_utils


class TestExtractUnwindTables(unittest.TestCase):
  def testExtractCfi(self):
    with tempfile.NamedTemporaryFile() as input_file, \
        tempfile.NamedTemporaryFile() as output_file:
      input_file.write("""
MODULE Linux arm CDE12FE1DF2B37A9C6560B4CBEE056420 lib_chrome.so
INFO CODE_ID E12FE1CD2BDFA937C6560B4CBEE05642
FILE 0 ../../base/allocator/allocator_check.cc
FILE 1 ../../base/allocator/allocator_extension.cc
FILE 2 ../../base/allocator/allocator_shim.cc
FUNC 1adcb60 54 0 i2d_name_canon
1adcb60 1a 509 17054
3b94c70 2 69 40
PUBLIC e17001 0 assist_ranker::(anonymous namespace)::FakePredict::Initialize()
PUBLIC e17005 0 (anonymous namespace)::FileDeleter(base::File)
STACK CFI INIT e17000 4 .cfa: sp 0 + .ra: lr
STACK CFI INIT 0 4 .cfa: sp 0 + .ra: lr
STACK CFI 2 .cfa: sp 4 +
STACK CFI 4 .cfa: sp 12 + .ra: .cfa -8 + ^ r7: .cfa -12 + ^
STACK CFI 6 .cfa: sp 16 +
STACK CFI INIT e1a96e 20 .cfa: sp 0 + .ra: lr
STACK CFI e1a970 .cfa: sp 4 +
STACK CFI e1a972 .cfa: sp 12 + .ra: .cfa -8 + ^ r7: .cfa -12 + ^
STACK CFI e1a974 .cfa: sp 16 +
STACK CFI INIT e1a1e4 b0 .cfa: sp 0 + .ra: lr
STACK CFI e1a1e6 .cfa: sp 16 + .ra: .cfa -4 + ^ r4: .cfa -16 + ^ r5: .cfa -12 +
STACK CFI e1a1e8 .cfa: sp 80 +
STACK CFI INIT 0 4 .cfa: sp 0 + .ra: lr
STACK CFI INIT 3b92e24 3c .cfa: sp 0 + .ra: lr
STACK CFI 3b92e4c .cfa: sp 16 + .ra: .cfa -12 + ^
STACK CFI INIT e17004 0 .cfa: sp 0 + .ra: lr
STACK CFI e17004 2 .cfa: sp 0 + .ra: lr
STACK CFI INIT 3b92e70 38 .cfa: sp 0 + .ra: lr
STACK CFI 3b92e74 .cfa: sp 8 + .ra: .cfa -4 + ^ r4: .cfa -8 + ^
STACK CFI 3b92e90 .cfa: sp 0 + .ra: .ra r4: r4
STACK CFI INIT 3b93114 6c .cfa: sp 0 + .ra: lr
STACK CFI 3b93118 .cfa: r7 16 + .ra: .cfa -4 + ^
STACK CFI INIT 3b92114 6c .cfa: sp 0 + .ra: lr
STACK CFI 3b92118 .cfa: r7 16 + .ra: .cfa -20 + ^
STACK CFI INIT 3b93214 fffff .cfa: sp 0 + .ra: lr
STACK CFI 3b93218 .cfa: r7 16 + .ra: .cfa -4 + ^
""")
      input_file.flush()
      extract_unwind_tables._ParseCfiData(input_file.name, output_file.name)

      expected_cfi_data = {
        0xe1a1e4 : [0x2, 0x11, 0x4, 0x50],
        0xe1a296 : [],
        0xe1a96e : [0x2, 0x4, 0x4, 0xe, 0x6, 0x10],
        0xe1a990 : [],
        0x3b92e24: [0x28, 0x13],
        0x3b92e62: [],
      }
      expected_function_count = len(expected_cfi_data)

      actual_output = []
      with open(output_file.name, 'rb') as f:
        while True:
          read = f.read(2)
          if not read:
            break
          actual_output.append(struct.unpack('H', read)[0])

      # First value is size of unw_index table.
      unw_index_size = actual_output[1] << 16 | actual_output[0]
      # Each function index is 6 bytes data.
      self.assertEqual(expected_function_count * 6, unw_index_size)
      # |actual_output| is in blocks of 2 bytes. Skip first 4 bytes representing
      # size.
      unw_index_start = 2
      unw_index_addr_end = unw_index_start + expected_function_count * 2
      unw_index_end = unw_index_addr_end + expected_function_count
      unw_index_addr_col = actual_output[unw_index_start : unw_index_addr_end]
      unw_index_index_col = actual_output[unw_index_addr_end : unw_index_end]

      unw_data_start = unw_index_end
      unw_data = actual_output[unw_data_start:]

      for func_iter in range(0, expected_function_count):
        func_addr = (unw_index_addr_col[func_iter * 2 + 1] << 16 |
                     unw_index_addr_col[func_iter * 2])
        index = unw_index_index_col[func_iter]
        # If index is CANT_UNWIND then invalid function.
        if index == 0xFFFF:
          self.assertEqual(expected_cfi_data[func_addr], [])
          continue

        func_start = index + 1
        func_end = func_start + unw_data[index] * 2
        self.assertEquals(
            len(expected_cfi_data[func_addr]), func_end - func_start)
        func_cfi = unw_data[func_start : func_end]
        self.assertEqual(expected_cfi_data[func_addr], func_cfi)


if __name__ == '__main__':
  unittest.main()
