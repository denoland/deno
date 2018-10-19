# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import sys
import os
import benchmark


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
    if "linux" in sys.platform:
        thread_count_test(deno_path)
        syscall_count_test(deno_path)
