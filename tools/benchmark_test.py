import sys
import os
from benchmark import run_thread_count_benchmark


def benchmark_test(deno_path):
    if "linux" in sys.platform:
        thread_count_dict = run_thread_count_benchmark(deno_path)
        assert "set_timeout" in thread_count_dict
        assert thread_count_dict["set_timeout"] > 1
