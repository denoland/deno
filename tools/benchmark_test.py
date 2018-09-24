#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import sys
import os
from util import build_path
from benchmark import get_build_dir_from_argv, run_thread_count_tests


def main(argv):
    build_dir = get_build_dir_from_argv(argv)
    deno_path = os.path.join(build_dir, "deno")

    if "linux" in sys.platform:
        thread_count_dict = run_thread_count_tests(deno_path)
        assert "set_timeout" in thread_count_dict
        assert thread_count_dict["set_timeout"] > 1


if __name__ == '__main__':
    main(sys.argv)
