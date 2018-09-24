import sys
import os
from benchmark import run_thread_count_benchmark, run_syscall_count_benchmark


def thread_count_test(deno_path):
    thread_count_dict = run_thread_count_benchmark(deno_path)
    assert "set_timeout" in thread_count_dict
    assert thread_count_dict["set_timeout"] > 1


def syscall_count_test(deno_path):
    syscall_count_dict = run_syscall_count_benchmark(deno_path)
    assert "hello" in syscall_count_dict
    assert syscall_count_dict["hello"] > 1


def benchmark_test(deno_path):
    if "linux" in sys.platform:
        thread_count_test(deno_path)
        syscall_count_test(deno_path)
