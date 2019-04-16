#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import sys
import os
import benchmark
from util import build_path, executable_suffix


def strace_parse_test():
    with open(os.path.join(sys.path[0], "testdata/strace_summary.out"),
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


def max_mem_parse_test():
    with open(os.path.join(sys.path[0], "testdata/time.out"), "r") as f:
        data = f.read()
        assert benchmark.find_max_mem_in_bytes(data) == 120380 * 1024


def binary_size_test(build_dir):
    binary_size_dict = benchmark.get_binary_sizes(build_dir)
    assert binary_size_dict["deno"] > 0
    assert binary_size_dict["main.js"] > 0
    assert binary_size_dict["main.js.map"] > 0
    assert binary_size_dict["snapshot_deno.bin"] > 0


def thread_count_test(deno_path):
    thread_count_dict = benchmark.run_thread_count_benchmark(deno_path)
    assert "set_timeout" in thread_count_dict
    assert thread_count_dict["set_timeout"] > 1


def syscall_count_test(deno_path):
    syscall_count_dict = benchmark.run_syscall_count_benchmark(deno_path)
    assert "hello" in syscall_count_dict
    assert syscall_count_dict["hello"] > 1


def benchmark_test(build_dir, deno_path):
    strace_parse_test()
    binary_size_test(build_dir)
    max_mem_parse_test()
    if "linux" in sys.platform:
        thread_count_test(deno_path)
        syscall_count_test(deno_path)


# This test assumes tools/http_server.py is running in the background.
def main():
    if len(sys.argv) == 2:
        build_dir = sys.argv[1]
    elif len(sys.argv) == 1:
        build_dir = build_path()
    else:
        print "Usage: tools/benchmark_test.py [build_dir]"
        sys.exit(1)
    deno_exe = os.path.join(build_dir, "deno" + executable_suffix)
    benchmark_test(build_dir, deno_exe)


if __name__ == '__main__':
    main()
