# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import logging
import os
import subprocess
import threading
import time
import uuid

from devil.utils import reraiser_thread
from pylib import constants


_MINIUMUM_TIMEOUT = 3.0
_PER_LINE_TIMEOUT = .002  # Should be able to process 500 lines per second.
_PROCESS_START_TIMEOUT = 10.0


class Deobfuscator(object):
  def __init__(self, mapping_path):
    script_path = os.path.join(
        constants.GetOutDirectory(), 'bin', 'java_deobfuscate')
    cmd = [script_path, mapping_path]
    # Allow only one thread to call TransformLines() at a time.
    self._lock = threading.Lock()
    # Ensure that only one thread attempts to kill self._proc in Close().
    self._close_lock = threading.Lock()
    self._closed_called = False
    # Assign to None so that attribute exists if Popen() throws.
    self._proc = None
    # Start process eagerly to hide start-up latency.
    self._proc_start_time = time.time()
    self._proc = subprocess.Popen(
        cmd, bufsize=1, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        close_fds=True)

  def IsClosed(self):
    return self._closed_called or self._proc.returncode is not None

  def IsBusy(self):
    return self._lock.locked()

  def IsReady(self):
    return not self.IsClosed() and not self.IsBusy()

  def TransformLines(self, lines):
    """Deobfuscates obfuscated names found in the given lines.

    If anything goes wrong (process crashes, timeout, etc), returns |lines|.

    Args:
      lines: A list of strings without trailing newlines.

    Returns:
      A list of strings without trailing newlines.
    """
    if not lines:
      return []

    # Deobfuscated stacks contain more frames than obfuscated ones when method
    # inlining occurs. To account for the extra output lines, keep reading until
    # this eof_line token is reached.
    eof_line = uuid.uuid4().hex
    out_lines = []

    def deobfuscate_reader():
      while True:
        line = self._proc.stdout.readline()
        # Return an empty string at EOF (when stdin is closed).
        if not line:
          break
        line = line[:-1]
        if line == eof_line:
          break
        out_lines.append(line)

    if self.IsBusy():
      logging.warning('deobfuscator: Having to wait for Java deobfuscation.')

    # Allow only one thread to operate at a time.
    with self._lock:
      if self.IsClosed():
        if not self._closed_called:
          logging.warning('deobfuscator: Process exited with code=%d.',
                          self._proc.returncode)
          self.Close()
        return lines

      # TODO(agrieve): Can probably speed this up by only sending lines through
      #     that might contain an obfuscated name.
      reader_thread = reraiser_thread.ReraiserThread(deobfuscate_reader)
      reader_thread.start()

      try:
        self._proc.stdin.write('\n'.join(lines))
        self._proc.stdin.write('\n{}\n'.format(eof_line))
        self._proc.stdin.flush()
        time_since_proc_start = time.time() - self._proc_start_time
        timeout = (max(0, _PROCESS_START_TIMEOUT - time_since_proc_start) +
                   max(_MINIUMUM_TIMEOUT, len(lines) * _PER_LINE_TIMEOUT))
        reader_thread.join(timeout)
        if self.IsClosed():
          logging.warning(
              'deobfuscator: Close() called by another thread during join().')
          return lines
        if reader_thread.is_alive():
          logging.error('deobfuscator: Timed out.')
          self.Close()
          return lines
        return out_lines
      except IOError:
        logging.exception('deobfuscator: Exception during java_deobfuscate')
        self.Close()
        return lines

  def Close(self):
    with self._close_lock:
      needs_closing = not self.IsClosed()
      self._closed_called = True

    if needs_closing:
      self._proc.stdin.close()
      self._proc.kill()
      self._proc.wait()

  def __del__(self):
    # self._proc is None when Popen() fails.
    if not self._closed_called and self._proc:
      logging.error('deobfuscator: Forgot to Close()')
      self.Close()


class DeobfuscatorPool(object):
  # As of Sep 2017, each instance requires about 500MB of RAM, as measured by:
  # /usr/bin/time -v out/Release/bin/java_deobfuscate \
  #     out/Release/apks/ChromePublic.apk.mapping
  def __init__(self, mapping_path, pool_size=4):
    self._mapping_path = mapping_path
    self._pool = [Deobfuscator(mapping_path) for _ in xrange(pool_size)]
    # Allow only one thread to select from the pool at a time.
    self._lock = threading.Lock()

  def TransformLines(self, lines):
    with self._lock:
      assert self._pool, 'TransformLines() called on a closed DeobfuscatorPool.'
      # Restart any closed Deobfuscators.
      for i, d in enumerate(self._pool):
        if d.IsClosed():
          logging.warning('deobfuscator: Restarting closed instance.')
          self._pool[i] = Deobfuscator(self._mapping_path)

      selected = next((x for x in self._pool if x.IsReady()), self._pool[0])
      # Rotate the order so that next caller will not choose the same one.
      self._pool.remove(selected)
      self._pool.append(selected)

    return selected.TransformLines(lines)

  def Close(self):
    with self._lock:
      for d in self._pool:
        d.Close()
      self._pool = None
