#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
import sys
from util import build_path
from benchmark import read_json, write_json


def main(argv):
    if len(argv) == 2:
        build_dir = sys.argv[1]
    elif len(argv) == 1:
        build_dir = build_path()
    else:
        print "Usage: tools/build_benchmark_jsons.py [build_dir]"
        sys.exit(1)

    current_data_file = os.path.join(build_dir, "bench.json")
    all_data_file = "gh-pages/data.json"  # Includes all benchmark data.
    recent_data_file = "gh-pages/recent.json"  # Has 20 most recent results.

    assert os.path.exists(current_data_file)
    assert os.path.exists(all_data_file)

    new_data = read_json(current_data_file)
    all_data = read_json(all_data_file)
    all_data.append(new_data)

    write_json(all_data_file, all_data)
    write_json(recent_data_file, all_data[-20:])


if __name__ == '__main__':
    main(sys.argv)
