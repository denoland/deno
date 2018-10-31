#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# Given a deno executable, this script executes several integration tests also
# passing command line flags to the deno executable based on the path of the
# test case.
#
# Usage: flag_output_tests.py [path to deno executable]
import os
import sys
import subprocess
from util import pattern_match, parse_exit_code

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
flags_path = os.path.join(root_path, "tests", "flags")


def flag_output_tests(deno_executable):
    assert os.path.isfile(deno_executable)
    switch_dirs = sorted([
        filename for filename in os.listdir(flags_path)
        if os.path.isdir(os.path.join(flags_path, filename))
    ])
    for switch_dir in switch_dirs:
        tests_path = os.path.join(flags_path, switch_dir)
        outs = sorted([
            filename for filename in os.listdir(tests_path)
            if filename.endswith(".out")
        ])
        assert len(outs) > 0
        tests = [(os.path.splitext(filename)[0], filename)
                 for filename in outs]
        for (script, out_filename) in tests:
            script_abs = os.path.join(tests_path, script)
            out_abs = os.path.join(tests_path, out_filename)
            with open(out_abs, 'r') as f:
                expected_out = f.read()
            flags = ["--" + flag for flag in switch_dir.split("_")]
            cmd = [deno_executable, script_abs, "--reload"] + flags
            expected_code = parse_exit_code(script)
            print " ".join(cmd)
            actual_code = 0
            try:
                actual_out = subprocess.check_output(
                    cmd, universal_newlines=True)
            except subprocess.CalledProcessError as e:
                actual_code = e.returncode
                actual_out = e.output
                if expected_code == 0:
                    print "Expected success but got error. Output:"
                    print actual_out
                    sys.exit(1)

            if expected_code != actual_code:
                print "Expected exit code %d but got %d" % (expected_code,
                                                            actual_code)
                print "Output:"
                print actual_out
                sys.exit(1)

            if pattern_match(expected_out, actual_out) != True:
                print "Expected output does not match actual."
                print "Expected: " + expected_out
                print "Actual:   " + actual_out
                sys.exit(1)


def main(argv):
    flag_output_tests(argv[1])


if __name__ == '__main__':
    sys.exit(main(sys.argv))
