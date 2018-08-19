#!/usr/bin/env python
# Given a deno executable, this script execute several integration tests
# with it. The tests are stored in //tests/ and each script has a corresponding
# .out file which specifies what the stdout should be.
#
# Usage: check_output_test.py [path to deno executable]
import os
import sys
import subprocess
from util import pattern_match, parse_exit_code

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
tests_path = os.path.join(root_path, "tests")


def check_output_test(deno_exe_filename):
    assert os.path.isfile(deno_exe_filename)
    outs = sorted([
        filename for filename in os.listdir(tests_path)
        if filename.endswith(".out")
    ])
    assert len(outs) > 1
    tests = [(os.path.splitext(filename)[0], filename) for filename in outs]
    for (script, out_filename) in tests:
        script_abs = os.path.join(tests_path, script)
        out_abs = os.path.join(tests_path, out_filename)
        with open(out_abs, 'r') as f:
            expected_out = f.read()
        cmd = [deno_exe_filename, script_abs]
        expected_code = parse_exit_code(script)
        print " ".join(cmd)
        actual_code = 0
        try:
            actual_out = subprocess.check_output(cmd, universal_newlines=True)
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
    check_output_test(argv[1])


if __name__ == '__main__':
    sys.exit(main(sys.argv))
