#!/usr/bin/env python
#
# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""This script creates a "jumbo" file which merges all incoming files
for compiling.

"""

from __future__ import print_function

import argparse
import hashlib
import cStringIO
import os

def cut_ranges(boundaries):
  # Given an increasing sequence of boundary indices, generate a sequence of
  # non-overlapping ranges. The total range is inclusive of the first index
  # and exclusive of the last index from the given sequence.
  for start, stop in zip(boundaries, boundaries[1:]):
    yield range(start, stop)


def generate_chunk_stops(inputs, output_count, smart_merge=True):
  # Note: In the comments below, unique numeric labels are assigned to files.
  #       Consider them as the sorted rank of the hash of each file path.
  # Simple jumbo chunking generates uniformly sized chunks with the ceiling of:
  # (output_index + 1) * input_count / output_count
  input_count = len(inputs)
  stops = [((i + 1) * input_count + output_count - 1) // output_count
           for i in range(output_count)]
  # This is disruptive at times because file insertions and removals can
  # invalidate many chunks as all files are offset by one.
  # For example, say we have 12 files in 4 uniformly sized chunks:
  # 9, 4, 0; 7,  1, 11;  5, 10, 2; 6, 3, 8
  # If we delete the first file we get:
  # 4, 0, 7; 1, 11,  5; 10,  2, 6; 3, 8
  # All of the chunks have new sets of inputs.

  # With path-aware chunking, we start with the uniformly sized chunks:
  # 9, 4, 0; 7,  1, 11;  5, 10, 2; 6, 3, 8
  # First we find the smallest rank in each of the chunks. Their indices are
  # stored in the |centers| list and in this example the ranks would be:
  # 0, 1, 2, 3
  # Then we find the largest rank between the centers. Their indices are stored
  # in the |stops| list and in this example the ranks would be:
  # 7, 11, 6
  # These files mark the boundaries between chunks and these boundary files are
  # often maintained even as files are added or deleted.
  # In this example, 7, 11, and 6 are the first files in each chunk:
  # 9, 4, 0; 7,  1; 11,  5, 10, 2; 6, 3, 8
  # If we delete the first file and repeat the process we get:
  # 4, 0; 7, 1; 11,  5, 10,  2; 6, 3, 8
  # Only the first chunk has a new set of inputs.
  if smart_merge:
    # Starting with the simple chunks, every file is assigned a rank.
    # This requires a hash function that is stable across runs.
    hasher = lambda n: hashlib.md5(inputs[n]).hexdigest()
    # In each chunk there is a key file with lowest rank; mark them.
    # Note that they will not easily change.
    centers = [min(indices, key=hasher) for indices in cut_ranges([0] + stops)]
    # Between each pair of key files there is a file with highest rank.
    # Mark these to be used as border files. They also will not easily change.
    # Forget the inital chunks and create new chunks by splitting the list at
    # every border file.
    stops = [max(indices, key=hasher) for indices in cut_ranges(centers)]
    stops.append(input_count)
  return stops


def write_jumbo_files(inputs, outputs, written_input_set, written_output_set):
  chunk_stops = generate_chunk_stops(inputs, len(outputs))

  written_inputs = 0
  for output_index, output_file in enumerate(outputs):
    written_output_set.add(output_file)
    if os.path.isfile(output_file):
      with open(output_file, "r") as current:
        current_jumbo_file = current.read()
    else:
      current_jumbo_file = None

    out = cStringIO.StringIO()
    out.write("/* This is a Jumbo file. Don't edit. */\n\n")
    out.write("/* Generated with merge_for_jumbo.py. */\n\n")
    input_limit = chunk_stops[output_index]
    while written_inputs < input_limit:
      filename = inputs[written_inputs]
      written_inputs += 1
      out.write("#include \"%s\"\n" % filename)
      written_input_set.add(filename)
    new_jumbo_file = out.getvalue()
    out.close()

    if new_jumbo_file != current_jumbo_file:
      with open(output_file, "w") as out:
        out.write(new_jumbo_file)


def main():
  parser = argparse.ArgumentParser()
  parser.add_argument("--outputs", nargs="+", required=True,
                      help='List of output files to split input into')
  parser.add_argument("--file-list", required=True)
  parser.add_argument("--verbose", action="store_true")
  args = parser.parse_args()

  lines = []
  # If written with gn |write_file| each file is on its own line.
  with open(args.file_list) as file_list_file:
    lines = [line.strip() for line in file_list_file if line.strip()]
  # If written with gn |response_file_contents| the files are space separated.
  all_inputs = []
  for line in lines:
    all_inputs.extend(line.split())

  written_output_set = set()  # Just for double checking
  written_input_set = set()  # Just for double checking
  for language_ext in (".cc", ".c", ".mm", ".S"):
    if language_ext == ".cc":
      ext_pattern = (".cc", ".cpp")
    else:
      ext_pattern = tuple([language_ext])

    outputs = [x for x in args.outputs if x.endswith(ext_pattern)]
    inputs = [x for x in all_inputs if x.endswith(ext_pattern)]

    if not outputs:
      assert not inputs
      continue

    write_jumbo_files(inputs, outputs, written_input_set, written_output_set)

  assert set(args.outputs) == written_output_set, "Did not fill all outputs"
  if args.verbose:
    print("Generated %s (%d files) based on %s" % (
      str(args.outputs), len(written_input_set), args.file_list))

if __name__ == "__main__":
  main()
