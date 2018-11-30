#!/usr/bin/python
# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import qemu_target
import shutil
import subprocess
import tempfile
import time
import unittest

TEST_PAYLOAD = "Let's get this payload across the finish line!"

tmpdir = tempfile.mkdtemp()

# Register the target with the context manager so that it always gets
# torn down on process exit. Otherwise there might be lingering QEMU instances
# if Python crashes or is interrupted.
with qemu_target.QemuTarget(tmpdir, 'x64') as target:
  class TestQemuTarget(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
      target.Start()

    @classmethod
    def tearDownClass(cls):
      target.Shutdown()
      shutil.rmtree(tmpdir)

    def testCopyBidirectional(self):
      tmp_path = tmpdir + "/payload"
      with open(tmp_path, "w") as tmpfile:
        tmpfile.write(TEST_PAYLOAD)
      target.PutFile(tmp_path, '/tmp/payload')

      tmp_path_roundtrip = tmp_path + ".roundtrip"
      target.GetFile('/tmp/payload', tmp_path_roundtrip)
      with open(tmp_path_roundtrip) as roundtrip:
        self.assertEqual(TEST_PAYLOAD, roundtrip.read())

    def testRunCommand(self):
      self.assertEqual(0, target.RunCommand(['true']))

      # This is a known bug: https://fuchsia.atlassian.net/browse/NET-349
      self.assertEqual(1, target.RunCommand(['false']))

    def testRunCommandPiped(self):
      proc = target.RunCommandPiped(['cat'],
                                    stdin=subprocess.PIPE,
                                    stdout=subprocess.PIPE)
      proc.stdin.write(TEST_PAYLOAD)
      proc.stdin.flush()
      proc.stdin.close()
      self.assertEqual(TEST_PAYLOAD, proc.stdout.readline())
      proc.kill()


  if __name__ == '__main__':
      unittest.main()
