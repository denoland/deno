# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging
import os
import re
import subprocess

# Matches the coarse syntax of a backtrace entry.
_BACKTRACE_PREFIX_RE = re.compile(r'(\[[0-9.]+\] )?bt#(?P<frame_id>\d+): ')

# Matches the specific fields of a backtrace entry.
# Back-trace line matcher/parser assumes that 'pc' is always present, and
# expects that 'sp' and ('binary','pc_offset') may also be provided.
_BACKTRACE_ENTRY_RE = re.compile(
    r'pc 0(?:x[0-9a-f]+)?' +
    r'(?: sp 0x[0-9a-f]+)?' +
    r'(?: \((?P<binary>\S+),(?P<pc_offset>0x[0-9a-f]+)\))?$')


def _GetUnstrippedPath(path):
  """If there is a binary located at |path|, returns a path to its unstripped
  source.

  Returns None if |path| isn't a binary or doesn't exist in the lib.unstripped
  or exe.unstripped directories."""

  if path.endswith('.so'):
    maybe_unstripped_path = os.path.normpath(
        os.path.join(path, os.path.pardir, 'lib.unstripped',
                     os.path.basename(path)))
  else:
    maybe_unstripped_path = os.path.normpath(
        os.path.join(path, os.path.pardir, 'exe.unstripped',
                     os.path.basename(path)))

  if not os.path.exists(maybe_unstripped_path):
    return None

  with open(maybe_unstripped_path, 'rb') as f:
    file_tag = f.read(4)
  if file_tag != '\x7fELF':
    logging.warn('Expected an ELF binary: ' + maybe_unstripped_path)
    return None

  return maybe_unstripped_path


def FilterStream(stream, package_name, manifest_path, output_dir):
  """Looks for backtrace lines from an iterable |stream| and symbolizes them.
  Yields a stream of strings with symbolized entries replaced."""

  return _SymbolizerFilter(package_name,
                           manifest_path,
                           output_dir).SymbolizeStream(stream)


class _SymbolizerFilter(object):
  """Adds backtrace symbolization capabilities to a process output stream."""

  def __init__(self, package_name, manifest_path, output_dir):
    self._symbols_mapping = {}
    self._output_dir = output_dir
    self._package_name = package_name

    # Compute remote/local path mappings using the manifest data.
    for next_line in open(manifest_path):
      target, source = next_line.strip().split('=')
      stripped_binary_path = _GetUnstrippedPath(os.path.join(output_dir,
                                                             source))
      if not stripped_binary_path:
        continue

      self._symbols_mapping[os.path.basename(target)] = stripped_binary_path
      self._symbols_mapping[target] = stripped_binary_path
      if target == 'bin/app':
        self._symbols_mapping[package_name] = stripped_binary_path
      logging.debug('Symbols: %s -> %s' % (source, target))

  def _SymbolizeEntries(self, entries):
    """Symbolizes the parsed backtrace |entries| by calling addr2line.

    Returns a set of (frame_id, result) pairs."""

    filename_re = re.compile(r'at ([-._a-zA-Z0-9/+]+):(\d+)')

    # Use addr2line to symbolize all the |pc_offset|s in |entries| in one go.
    # Entries with no |debug_binary| are also processed here, so that we get
    # consistent output in that case, with the cannot-symbolize case.
    addr2line_output = None
    if entries[0].has_key('debug_binary'):
      addr2line_args = (['addr2line', '-Cipf', '-p',
                        '--exe=' + entries[0]['debug_binary']] +
                        map(lambda entry: entry['pc_offset'], entries))
      addr2line_output = subprocess.check_output(addr2line_args).splitlines()
      assert addr2line_output

    results = {}
    for entry in entries:
      raw, frame_id = entry['raw'], entry['frame_id']
      prefix = '#%s: ' % frame_id

      if not addr2line_output:
        # Either there was no addr2line output, or too little of it.
        filtered_line = raw
      else:
        output_line = addr2line_output.pop(0)

        # Relativize path to the current working (output) directory if we see
        # a filename.
        def RelativizePath(m):
          relpath = os.path.relpath(os.path.normpath(m.group(1)))
          return 'at ' + relpath + ':' + m.group(2)
        filtered_line = filename_re.sub(RelativizePath, output_line)

        if '??' in filtered_line.split():
          # If symbolization fails just output the raw backtrace.
          filtered_line = raw
        else:
          # Release builds may inline things, resulting in "(inlined by)" lines.
          inlined_by_prefix = " (inlined by)"
          while (addr2line_output and
                 addr2line_output[0].startswith(inlined_by_prefix)):
            inlined_by_line = \
                '\n' + (' ' * len(prefix)) + addr2line_output.pop(0)
            filtered_line += filename_re.sub(RelativizePath, inlined_by_line)

      results[entry['frame_id']] = prefix + filtered_line

    return results

  def _LookupDebugBinary(self, entry):
    """Looks up the binary listed in |entry| in the |_symbols_mapping|.
    Returns the corresponding host-side binary's filename, or None."""

    binary = entry['binary']
    if not binary:
      return None

    app_prefix = 'app:'
    if binary.startswith(app_prefix):
      binary = binary[len(app_prefix):]

    # We change directory into /system/ before running the target executable, so
    # all paths are relative to "/system/", and will typically start with "./".
    # Some crashes still uses the full filesystem path, so cope with that, too.
    pkg_prefix = '/pkg/'
    cwd_prefix = './'
    if binary.startswith(cwd_prefix):
      binary = binary[len(cwd_prefix):]
    elif binary.startswith(pkg_prefix):
      binary = binary[len(pkg_prefix):]
    # Allow other paths to pass-through; sometimes neither prefix is present.

    if binary in self._symbols_mapping:
      return self._symbols_mapping[binary]

    # |binary| may be truncated by the crashlogger, so if there is a unique
    # match for the truncated name in |symbols_mapping|, use that instead.
    matches = filter(lambda x: x.startswith(binary),
                               self._symbols_mapping.keys())
    if len(matches) == 1:
      return self._symbols_mapping[matches[0]]

    return None

  def _SymbolizeBacktrace(self, backtrace):
    """Group |backtrace| entries according to the associated binary, and locate
    the path to the debug symbols for that binary, if any."""

    batches = {}

    for entry in backtrace:
      debug_binary = self._LookupDebugBinary(entry)
      if debug_binary:
        entry['debug_binary'] = debug_binary
      batches.setdefault(debug_binary, []).append(entry)

    # Run _SymbolizeEntries on each batch and collate the results.
    symbolized = {}
    for batch in batches.itervalues():
      symbolized.update(self._SymbolizeEntries(batch))

    # Map each entry to its symbolized form, by frame-id, and return the list.
    return map(lambda entry: symbolized[entry['frame_id']], backtrace)

  def SymbolizeStream(self, stream):
    """Creates a symbolized logging stream object using the output from
    |stream|."""

    # A buffer of backtrace entries awaiting symbolization, stored as dicts:
    # raw: The original back-trace line that followed the prefix.
    # frame_id: backtrace frame number (starting at 0).
    # binary: path to executable code corresponding to the current frame.
    # pc_offset: memory offset within the executable.
    backtrace_entries = []

    # Read from the stream until we hit EOF.
    for line in stream:
      line = line.rstrip()

      # Look for the back-trace prefix, otherwise just emit the line.
      matched = _BACKTRACE_PREFIX_RE.match(line)
      if not matched:
        yield line
        continue
      backtrace_line = line[matched.end():]

      # If this was the end of a back-trace then symbolize and emit it.
      frame_id = matched.group('frame_id')
      if backtrace_line == 'end':
        if backtrace_entries:
          for processed in self._SymbolizeBacktrace(backtrace_entries):
            yield processed
        backtrace_entries = []
        continue

      # Parse the program-counter offset, etc into |backtrace_entries|.
      matched = _BACKTRACE_ENTRY_RE.match(backtrace_line)
      if matched:
        # |binary| and |pc_offset| will be None if not present.
        backtrace_entries.append(
            {'raw': backtrace_line, 'frame_id': frame_id,
             'binary': matched.group('binary'),
             'pc_offset': matched.group('pc_offset')})
      else:
        backtrace_entries.append(
            {'raw': backtrace_line, 'frame_id': frame_id,
             'binary': None, 'pc_offset': None})
