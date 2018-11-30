#!/usr/bin/env python
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Extracts the unwind tables in from breakpad symbol files

Runs dump_syms on the given binary file and extracts the CFI data into the
given output file.
The output file is a binary file containing CFI rows ordered based on function
address. The output file only contains rows that match the most popular rule
type in CFI table, to reduce the output size and specify data in compact format.
See doc https://github.com/google/breakpad/blob/master/docs/symbol_files.md.
1. The CFA rules should be of postfix form "SP <val> +".
2. The RA rules should be of postfix form "CFA <val> + ^".
Note: breakpad represents dereferencing address with '^' operator.

The output file has 2 tables UNW_INDEX and UNW_DATA, inspired from ARM EHABI
format. The first table contains function addresses and an index into the
UNW_DATA table. The second table contains one or more rows for the function
unwind information.

The output file starts with 4 bytes counting the size of UNW_INDEX in bytes.
Then UNW_INDEX table and UNW_DATA table.

UNW_INDEX contains two columns of N rows each, where N is the number of
functions.
  1. First column 4 byte rows of all the function start address as offset from
     start of the binary, in sorted order.
  2. For each function addr, the second column contains 2 byte indices in order.
     The indices are offsets (in count of 2 bytes) of the CFI data from start of
     UNW_DATA.
The last entry in the table always contains CANT_UNWIND index to specify the
end address of the last function.

UNW_DATA contains data of all the functions. Each function data contains N rows.
The data found at the address pointed from UNW_INDEX will be:
  2 bytes: N - number of rows that belong to current function.
  N * 4 bytes: N rows of data. 16 bits : Address offset from function start.
                               14 bits : CFA offset / 4.
                                2 bits : RA offset / 4.

The function is not added to the unwind table in following conditions:
C1. If length of the function code (number of instructions) is greater than
    0xFFFF (2 byte address span). This is because we use 16 bits to refer to
    offset of instruction from start of the address.
C2. If the function moves the SP by more than 0xFFFF bytes. This is because we
    use 14 bits to denote CFA offset (last 2 bits are 0).
C3. If the Return Address is stored at an offset >= 16 from the CFA. Some
    functions which have variable arguments can have offset upto 16.
    TODO(ssid): We can actually store offset 16 by subtracting 1 from RA/4 since
    we never have 0.
C4: Some functions do not have unwind information defined in dwarf info. These
    functions have index value CANT_UNWIND(0xFFFF) in UNW_INDEX table.


Usage:
  extract_unwind_tables.py --input_path [root path to unstripped chrome.so]
      --output_path [output path] --dump_syms_path [path to dump_syms binary]
"""

import argparse
import re
import struct
import subprocess
import sys
import tempfile


_CFA_REG = '.cfa'
_RA_REG = '.ra'

_ADDR_ENTRY = 0
_LENGTH_ENTRY = 1

_CANT_UNWIND = 0xFFFF


def _Write4Bytes(output_file, val):
  """Writes a 32 bit unsigned integer to the given output file."""
  output_file.write(struct.pack('<L', val));


def _Write2Bytes(output_file, val):
  """Writes a 16 bit unsigned integer to the given output file."""
  output_file.write(struct.pack('<H', val));


def _FindRuleForRegister(cfi_row, reg):
  """Returns the postfix expression as string for a given register.

  Breakpad CFI row format specifies rules for unwinding each register in postfix
  expression form separated by space. Each rule starts with register name and a
  colon. Eg: "CFI R1: <rule> R2: <rule>".
  """
  out = []
  found_register = False
  for part in cfi_row:
    if found_register:
      if part[-1] == ':':
        break
      out.append(part)
    elif part == reg + ':':
      found_register = True
  return ' '.join(out)


def _GetCfaAndRaOffset(cfi_row):
  """Returns a tuple with 2 numbers (cfa_offset, ra_offset).

  Returns right values if rule matches the predefined criteria. Returns (0, 0)
  otherwise. The criteria for CFA rule is postfix form "SP <val> +" and RA rule
  is postfix form "CFA -<val> + ^".
  """
  cfa_offset = 0
  ra_offset = 0
  cfa_rule = _FindRuleForRegister(cfi_row, _CFA_REG)
  ra_rule = _FindRuleForRegister(cfi_row, _RA_REG)
  if cfa_rule and re.match(r'sp [0-9]+ \+', cfa_rule):
    cfa_offset = int(cfa_rule.split()[1], 10)
  if ra_rule:
    if not re.match(r'.cfa -[0-9]+ \+ \^', ra_rule):
      return (0, 0)
    ra_offset = -1 * int(ra_rule.split()[1], 10)
  return (cfa_offset, ra_offset)


def _GetAllCfiRows(symbol_file):
  """Returns parsed CFI data from given symbol_file.

  Each entry in the cfi data dictionary returned is a map from function start
  address to array of function rows, starting with FUNCTION type, followed by
  one or more CFI rows.
  """
  cfi_data = {}
  current_func = []
  for line in symbol_file:
    if 'STACK CFI' not in line:
      continue

    parts = line.split()
    data = {}
    if parts[2] == 'INIT':
      # Add the previous function to the output
      if len(current_func) > 1:
        cfi_data[current_func[0][_ADDR_ENTRY]] = current_func
      current_func = []

      # The function line is of format "STACK CFI INIT <addr> <length> ..."
      data[_ADDR_ENTRY] = int(parts[3], 16)
      data[_LENGTH_ENTRY] = int(parts[4], 16)

      # Condition C1: Skip if length is large.
      if data[_LENGTH_ENTRY] == 0 or data[_LENGTH_ENTRY] > 0xffff:
        continue  # Skip the current function.
    else:
      # The current function is skipped.
      if len(current_func) == 0:
        continue

      # The CFI row is of format "STACK CFI <addr> .cfa: <expr> .ra: <expr> ..."
      data[_ADDR_ENTRY] = int(parts[2], 16)
      (data[_CFA_REG], data[_RA_REG]) = _GetCfaAndRaOffset(parts)

      # Condition C2 and C3: Skip based on limits on offsets.
      if data[_CFA_REG] == 0 or data[_RA_REG] >= 16 or data[_CFA_REG] > 0xffff:
        current_func = []
        continue
      assert data[_CFA_REG] % 4 == 0
      # Since we skipped functions with code size larger than 0xffff, we should
      # have no function offset larger than the same value.
      assert data[_ADDR_ENTRY] - current_func[0][_ADDR_ENTRY] < 0xffff

    if data[_ADDR_ENTRY] == 0:
      # Skip current function, delete all previous entries.
      current_func = []
      continue
    assert data[_ADDR_ENTRY] % 2 == 0
    current_func.append(data)

  # Condition C4: Skip function without CFI rows.
  if len(current_func) > 1:
    cfi_data[current_func[0][_ADDR_ENTRY]] = current_func
  return cfi_data


def _WriteCfiData(cfi_data, out_file):
  """Writes the CFI data in defined format to out_file."""
  # Stores the final data that will be written to UNW_DATA table, in order
  # with 2 byte items.
  unw_data = []

  # Represent all the CFI data of functions as set of numbers and map them to an
  # index in the |unw_data|. This index is later written to the UNW_INDEX table
  # for each function. This map is used to find index of the data for functions.
  data_to_index = {}
  # Store mapping between the functions to the index.
  func_addr_to_index = {}
  previous_func_end = 0
  for addr, function in sorted(cfi_data.iteritems()):
    # Add an empty function entry when functions CFIs are missing between 2
    # functions.
    if previous_func_end != 0 and addr - previous_func_end  > 4:
      func_addr_to_index[previous_func_end + 2] = _CANT_UNWIND
    previous_func_end = addr + cfi_data[addr][0][_LENGTH_ENTRY]

    assert len(function) > 1
    func_data_arr = []
    func_data = 0
    # The first row contains the function address and length. The rest of the
    # rows have CFI data. Create function data array as given in the format.
    for row in function[1:]:
      addr_offset = row[_ADDR_ENTRY] - addr
      cfa_offset = (row[_CFA_REG]) | (row[_RA_REG] / 4)

      func_data_arr.append(addr_offset)
      func_data_arr.append(cfa_offset)

    # Consider all the rows in the data as one large integer and add it as a key
    # to the |data_to_index|.
    for data in func_data_arr:
      func_data = (func_data << 16) | data

    row_count = len(func_data_arr) / 2
    if func_data not in data_to_index:
      # When data is not found, create a new index = len(unw_data), and write
      # the data to |unw_data|.
      index = len(unw_data)
      data_to_index[func_data] = index
      unw_data.append(row_count)
      for row in func_data_arr:
        unw_data.append(row)
    else:
      # If the data was found, then use the same index for the function.
      index = data_to_index[func_data]
      assert row_count == unw_data[index]
    func_addr_to_index[addr] = data_to_index[func_data]

  # Mark the end end of last function entry.
  func_addr_to_index[previous_func_end + 2] = _CANT_UNWIND

  # Write the size of UNW_INDEX file in bytes.
  _Write4Bytes(out_file, len(func_addr_to_index) * 6)

  # Write the UNW_INDEX table. First list of addresses and then indices.
  sorted_unw_index = sorted(func_addr_to_index.iteritems())
  for addr, index in sorted_unw_index:
    _Write4Bytes(out_file, addr)
  for addr, index in sorted_unw_index:
    _Write2Bytes(out_file, index)

  # Write the UNW_DATA table.
  for data in unw_data:
    _Write2Bytes(out_file, data)


def _ParseCfiData(sym_file, output_path):
  with open(sym_file, 'r') as f:
    cfi_data =  _GetAllCfiRows(f)

  with open(output_path, 'wb') as out_file:
    _WriteCfiData(cfi_data, out_file)


def main():
  parser = argparse.ArgumentParser()
  parser.add_argument(
      '--input_path', required=True,
      help='The input path of the unstripped binary')
  parser.add_argument(
      '--output_path', required=True,
      help='The path of the output file')
  parser.add_argument(
      '--dump_syms_path', required=True,
      help='The path of the dump_syms binary')

  args = parser.parse_args()

  with tempfile.NamedTemporaryFile() as sym_file:
    out = subprocess.call(
        ['./' +args.dump_syms_path, args.input_path], stdout=sym_file)
    assert not out
    sym_file.flush()
    _ParseCfiData(sym_file.name, args.output_path)
  return 0

if __name__ == '__main__':
  sys.exit(main())
