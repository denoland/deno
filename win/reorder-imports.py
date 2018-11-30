#!/usr/bin/env python
# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import glob
import optparse
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..',
                                'third_party', 'pefile'))
import pefile

def reorder_imports(input_dir, output_dir, architecture):
  """Swap chrome_elf.dll to be the first import of chrome.exe.
  Also copy over any related files that might be needed
  (pdbs, manifests etc.).
  """
  # TODO(thakis): See if there is a reliable way to write the
  # correct executable in the first place, so that this script
  # only needs to verify that and not write a whole new exe.

  input_image = os.path.join(input_dir, 'chrome.exe')
  output_image = os.path.join(output_dir, 'chrome.exe')

  # pefile mmap()s the whole executable, and then parses parts of
  # it into python data structures for ease of processing.
  # To write the file again, only the mmap'd data is written back,
  # so modifying the parsed python objects generally has no effect.
  # However, parsed raw data ends up in pe.Structure instances,
  # and these all get serialized back when the file gets written.
  # So things that are in a Structure must have their data set
  # through the Structure, while other data must bet set through
  # the set_bytes_*() methods.
  pe = pefile.PE(input_image, fast_load=True)
  if architecture == 'x64':
    assert pe.PE_TYPE == pefile.OPTIONAL_HEADER_MAGIC_PE_PLUS
  else:
    assert pe.PE_TYPE == pefile.OPTIONAL_HEADER_MAGIC_PE

  pe.parse_data_directories(directories=[
      pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_IMPORT']])

  found_elf = False
  for i, peimport in enumerate(pe.DIRECTORY_ENTRY_IMPORT):
    if peimport.dll.lower() == 'chrome_elf.dll':
      assert not found_elf, 'only one chrome_elf.dll import expected'
      found_elf = True
      if i > 0:
        swap = pe.DIRECTORY_ENTRY_IMPORT[0]

        # Morally we want to swap peimport.struct and swap.struct here,
        # but the pe module doesn't expose a public method on Structure
        # to get all data of a Structure without explicitly listing all
        # field names.
        # NB: OriginalFirstThunk and Characteristics are an union both at
        # offset 0, handling just one of them is enough.
        peimport.struct.OriginalFirstThunk, swap.struct.OriginalFirstThunk = \
            swap.struct.OriginalFirstThunk, peimport.struct.OriginalFirstThunk
        peimport.struct.TimeDateStamp, swap.struct.TimeDateStamp = \
            swap.struct.TimeDateStamp, peimport.struct.TimeDateStamp
        peimport.struct.ForwarderChain, swap.struct.ForwarderChain = \
            swap.struct.ForwarderChain, peimport.struct.ForwarderChain
        peimport.struct.Name, swap.struct.Name = \
            swap.struct.Name, peimport.struct.Name
        peimport.struct.FirstThunk, swap.struct.FirstThunk = \
            swap.struct.FirstThunk, peimport.struct.FirstThunk
  assert found_elf, 'chrome_elf.dll import not found'

  pe.write(filename=output_image)

  for fname in glob.iglob(os.path.join(input_dir, 'chrome.exe.*')):
    shutil.copy(fname, os.path.join(output_dir, os.path.basename(fname)))
  return 0


def main(argv):
  usage = 'reorder_imports.py -i <input_dir> -o <output_dir> -a <target_arch>'
  parser = optparse.OptionParser(usage=usage)
  parser.add_option('-i', '--input', help='reorder chrome.exe in DIR',
      metavar='DIR')
  parser.add_option('-o', '--output', help='write new chrome.exe to DIR',
      metavar='DIR')
  parser.add_option('-a', '--arch', help='architecture of build (optional)',
      default='ia32')
  opts, args = parser.parse_args()

  if not opts.input or not opts.output:
    parser.error('Please provide and input and output directory')
  return reorder_imports(opts.input, opts.output, opts.arch)

if __name__ == "__main__":
  sys.exit(main(sys.argv[1:]))
