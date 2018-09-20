#!/usr/bin/env python
# Performs benchmark, and append data to //gh-pages/data.json.

import os
import sys
import json
import time
from util import run, run_output, root_path, build_path

benchmark_types = ["hello", "relative_import"]
benchmark_files = ["tests/002_hello.ts", "tests/003_relative_import.ts"]

data_file = "gh-pages/data.json"
benchmark_file = "benchmark.json"


def read_json(filename):
    with open(filename) as json_file:
        return json.load(json_file)


def write_json(filename, data):
    with open(filename, 'w') as outfile:
        json.dump(data, outfile)


def prepare_gh_pages_dir():
    if os.path.exists("gh-pages"):
        return
    try:
        run([
            "git", "clone", "--depth", "1", "-b", "gh-pages",
            "https://github.com/denoland/deno.git", "gh-pages"
        ])
    except:
        os.mkdir("gh-pages")
        with open("gh-pages/data.json", "w") as f:
            f.write("[]")  # writes empty json data


def main(argv):
    if len(argv) == 2:
        build_dir = sys.argv[1]
    elif len(argv) == 1:
        build_dir = build_path()
    else:
        print "Usage: tools/benchmark.py [build_dir]"
        sys.exit(1)

    os.chdir(root_path)
    prepare_gh_pages_dir()
    run(["hyperfine", "--export-json", benchmark_file, "--warmup", "3"] + [
        os.path.join(build_dir, "deno") + " " + file
        for file in benchmark_files
    ])
    all_data = read_json(data_file)
    benchmark_data = read_json(benchmark_file)
    sha1 = run_output(["git", "rev-parse", "HEAD"]).strip()
    new_data = {
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "sha1": sha1,
        "benchmark": {}
    }
    for type, data in zip(benchmark_types, benchmark_data["results"]):
        new_data["benchmark"][type] = {
            "mean": data["mean"],
            "stddev": data["stddev"],
            "user": data["user"],
            "system": data["system"],
            "min": data["min"],
            "max": data["max"]
        }
    all_data.append(new_data)
    write_json(data_file, all_data)


if __name__ == '__main__':
    main(sys.argv)
