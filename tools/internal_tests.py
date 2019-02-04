from setup_test import setup_test
from util_test import util_test
from benchmark_test import benchmark_test
from util import build_path, executable_suffix
import sys
import os

def internal_tests(build_dir, deno_exe):
  setup_test()
  util_test()
  benchmark_test(build_dir, deno_exe)

def main():
    import http_server
    http_server.spawn()

    build_dir = build_path()
    deno_exe = os.path.join(build_dir, "deno" + executable_suffix)

    internal_tests(build_dir, deno_exe)

if __name__ == "__main__":
    sys.exit(main())
