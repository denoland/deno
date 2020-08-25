#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import sys
import os
import unittest
import benchmark
from test_util import DenoTestCase, run_tests


class TestBenchmark(DenoTestCase):
    def test_strace_parse(self):
        with open(
                os.path.join(sys.path[0], "testdata/strace_summary.out"),
                "r") as f:
            summary = benchmark.strace_parse(f.read())
            # first syscall line
            assert summary["munmap"]["calls"] == 60
            assert summary["munmap"]["errors"] == 0
            # line with errors
            assert summary["mkdir"]["errors"] == 2
            # last syscall line
            assert summary["prlimit64"]["calls"] == 2
            assert summary["prlimit64"]["% time"] == 0
            # summary line
            assert summary["total"]["calls"] == 704

    def test_strace_parse2(self):
        with open(
                os.path.join(sys.path[0], "testdata/strace_summary2.out"),
                "r") as f:
            summary = benchmark.strace_parse(f.read())
            # first syscall line
            assert summary["futex"]["calls"] == 449
            assert summary["futex"]["errors"] == 94
            # summary line
            assert summary["total"]["calls"] == 821

    def test_max_mem_parse(self):
        with open(os.path.join(sys.path[0], "testdata/time.out"), "r") as f:
            data = f.read()
            assert benchmark.find_max_mem_in_bytes(data) == 120380 * 1024

    def test_binary_size(self):
        binary_size_dict = benchmark.get_binary_sizes(self.build_dir)
        assert binary_size_dict["deno"] > 0
        assert binary_size_dict["CLI_SNAPSHOT.bin"] > 0

    @unittest.skipIf("linux" not in sys.platform,
                     "strace only supported on linux")
    def test_strace(self):
        new_data = {}
        benchmark.run_strace_benchmarks(self.deno_exe, new_data)
        assert "thread_count" in new_data
        assert "syscall_count" in new_data

        s = new_data["thread_count"]
        assert "hello" in s
        assert s["hello"] > 1

        s = new_data["syscall_count"]
        assert "hello" in s
        assert s["hello"] > 1


if __name__ == '__main__':
    run_tests()
