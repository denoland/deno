#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import os
from util import build_path
from benchmark import read_json, write_json

current_data_file = os.path.join(build_path(), "bench.json")
all_data_file = "gh-pages/data.json"  # Includes all benchmark data.
recent_data_file = "gh-pages/recent.json"  # Includes recent 20 benchmark data.

assert os.path.exists(current_data_file)
assert os.path.exists(all_data_file)

new_data = read_json(current_data_file)
all_data = read_json(all_data_file)
all_data.append(new_data)

write_json(all_data_file, all_data)
write_json(recent_data_file, all_data[-20:])
