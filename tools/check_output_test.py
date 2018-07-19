#!/usr/bin/env python
# Usage: check_output_test.py [path to deno executable]
import os
import sys
import subprocess

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
tests_path = os.path.join(root_path, "tests")


def check_output_test(deno_fn):
    assert os.path.isfile(deno_fn)
    outs = [fn for fn in os.listdir(tests_path) if fn.endswith(".out")]
    assert len(outs) > 1
    tests = [(os.path.splitext(fn)[0], fn) for fn in outs]
    for (script, out_fn) in tests:
        script_abs = os.path.join(tests_path, script)
        out_abs = os.path.join(tests_path, out_fn)
        with open(out_abs, 'r') as f:
            expected_out = f.read()
        cmd = [deno_fn, script_abs]
        print " ".join(cmd)
        actual_out = subprocess.check_output(cmd)
        if expected_out != actual_out:
            print "Expected output does not match actual."
            sys.exit(1)


def main(argv):
    check_output_test(argv[1])


if __name__ == '__main__':
    sys.exit(main(sys.argv))
