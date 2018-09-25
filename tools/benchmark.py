#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# Performs benchmark and append data to //website/data.json.
# If //website/data.json doesn't exist, this script tries to import it from gh-pages branch.
# To view the results locally run ./tools/http_server.py and visit
# http://localhost:4545/website

import os
import sys
import json
import time
import shutil
from util import run, run_output, root_path, build_path
import tempfile

# The list of the tuples of the benchmark name and arguments
benchmarks = [("hello", ["tests/002_hello.ts", "--reload"]),
              ("relative_import", ["tests/003_relative_import.ts",
                                   "--reload"])]

gh_pages_data_file = "gh-pages/data.json"
data_file = "website/data.json"


def read_json(filename):
    with open(filename) as json_file:
        return json.load(json_file)


def write_json(filename, data):
    with open(filename, 'w') as outfile:
        json.dump(data, outfile)


def import_data_from_gh_pages():
    if os.path.exists(data_file):
        return
    try:
        run([
            "git", "clone", "--depth", "1", "-b", "gh-pages",
            "https://github.com/denoland/deno.git", "gh-pages"
        ])
        shutil.copy(gh_pages_data_file, data_file)
    except:
        write_json(data_file, [])  # writes empty json data


# run strace with test_args and record times a syscall record appears in out file
# based on syscall_line_matcher. Should be reusable
def count_strace_syscall(syscall_name, syscall_line_matcher, test_args):
    f = tempfile.NamedTemporaryFile()
    run(["strace", "-f", "-o", f.name, "-e", "trace=" + syscall_name] +
        test_args)
    return len(filter(syscall_line_matcher, f))


def run_thread_count_benchmark(deno_path):
    thread_count_map = {}
    thread_count_map["set_timeout"] = count_strace_syscall(
        "clone", lambda line: "clone(" in line,
        [deno_path, "tests/004_set_timeout.ts", "--reload"]) + 1
    return thread_count_map


def main(argv):
    if len(argv) == 2:
        build_dir = sys.argv[1]
    elif len(argv) == 1:
        build_dir = build_path()
    else:
        print "Usage: tools/benchmark.py [build_dir]"
        sys.exit(1)

    deno_path = os.path.join(build_dir, "deno")
    benchmark_file = os.path.join(build_dir, "benchmark.json")

    os.chdir(root_path)
    import_data_from_gh_pages()
    # TODO: Use hyperfine in //third_party
    run(["hyperfine", "--export-json", benchmark_file, "--warmup", "3"] +
        [deno_path + " " + " ".join(args) for [_, args] in benchmarks])
    all_data = read_json(data_file)
    benchmark_data = read_json(benchmark_file)
    sha1 = run_output(["git", "rev-parse", "HEAD"]).strip()
    new_data = {
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "sha1": sha1,
        "binary_size": os.path.getsize(deno_path),
        "thread_count": {},
        "benchmark": {}
    }
    for [[name, _], data] in zip(benchmarks, benchmark_data["results"]):
        new_data["benchmark"][name] = {
            "mean": data["mean"],
            "stddev": data["stddev"],
            "user": data["user"],
            "system": data["system"],
            "min": data["min"],
            "max": data["max"]
        }

    if "linux" in sys.platform:
        # Thread count test, only on linux
        new_data["thread_count"] = run_thread_count_benchmark(deno_path)

    all_data.append(new_data)
    write_json(data_file, all_data)


if __name__ == '__main__':
    main(sys.argv)
