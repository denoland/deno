#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

# This script is invoked independently of tools/test.py.
# TestCargoGn is purposely excluded from the main tests (tools/test.py) as it
# invokes a new build. It can be included if build.py is removed and "cargo
# build" becomes the main frontend.
# https://github.com/denoland/deno/issues/2608

import os
import unittest
from util import run_output


def touch(fname, times=None):
    with open(fname, 'a'):
        os.utime(fname, times)


# Warning: To avoid expensive rebuilds in CI, this should match the command
# invoked in travis and appveyor.
CARGO_CMD = ["cargo", "build", "-vv", "--release", "--locked"]


# Tests for cargo-gn integration.
class TestCargoGn(unittest.TestCase):
    def test_rerun_if_changed(self):
        # Here we are checking that "cargo build" does proper incremental
        # compilation. First we must run "cargo build" once, in case it hasn't
        # been done yet.
        print "[1/3] cargo build"
        result1 = run_output(CARGO_CMD, quiet=True)
        self.assertEqual(result1.code, 0)
        assert "Finished" in result1.err

        # Now we run it again, checking that there aren't any
        # cargo:rerun-if-changed lines in the output.
        print "[2/3] cargo build again..."
        result2 = run_output(CARGO_CMD, quiet=True)
        self.assertEqual(result2.code, 0)
        assert "Finished" in result2.err
        assert "cargo:rerun-if-changed" not in result2.out

        # Finally we touch some file that the build ought to depend on,
        # and check that the build does get rebuilt.
        touch("js/main.ts")
        print "[3/3] final cargo build"
        result3 = run_output(CARGO_CMD, quiet=True)
        self.assertEqual(result3.code, 0)
        assert "Finished" in result3.err
        assert "cargo:rerun-if-changed" in result3.out


if __name__ == '__main__':
    unittest.main()
