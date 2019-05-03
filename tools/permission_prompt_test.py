#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import pty
import select
import subprocess
import sys
import time

from util import build_path, executable_suffix, green_ok, red_failed

PERMISSIONS_PROMPT_TEST_TS = "tools/permission_prompt_test.ts"

PROMPT_PATTERN = b'⚠️'
FIRST_CHECK_FAILED_PATTERN = b'First check failed'
PERMISSION_DENIED_PATTERN = b'PermissionDenied: permission denied'


# This function is copied from:
# https://gist.github.com/hayd/4f46a68fc697ba8888a7b517a414583e
# https://stackoverflow.com/q/52954248/1240268
def tty_capture(cmd, bytes_input, timeout=5):
    """Capture the output of cmd with bytes_input to stdin,
    with stdin, stdout and stderr as TTYs."""
    mo, so = pty.openpty()  # provide tty to enable line-buffering
    me, se = pty.openpty()
    mi, si = pty.openpty()
    fdmap = {mo: 'stdout', me: 'stderr', mi: 'stdin'}

    timeout_exact = time.time() + timeout
    p = subprocess.Popen(
        cmd, bufsize=1, stdin=si, stdout=so, stderr=se, close_fds=True)
    os.write(mi, bytes_input)

    select_timeout = .04  #seconds
    res = {'stdout': b'', 'stderr': b''}
    while True:
        ready, _, _ = select.select([mo, me], [], [], select_timeout)
        if ready:
            for fd in ready:
                data = os.read(fd, 512)
                if not data:
                    break
                res[fdmap[fd]] += data
        elif p.poll() is not None or time.time(
        ) > timeout_exact:  # select timed-out
            break  # p exited
    for fd in [si, so, se, mi, mo, me]:
        os.close(fd)  # can't do it sooner: it leads to errno.EIO error
    p.wait()
    return p.returncode, res['stdout'], res['stderr']


# Wraps a test in debug printouts
# so we have visual indicator of what test failed
def wrap_test(test_name, test_method, *argv):
    sys.stdout.write(test_name + " ... ")
    try:
        test_method(*argv)
        print green_ok()
    except AssertionError:
        print red_failed()
        raise


class Prompt(object):
    def __init__(self, deno_exe, test_types):
        self.deno_exe = deno_exe
        self.test_types = test_types

    def run(self, flags, args, bytes_input):
        "Returns (return_code, stdout, stderr)."
        cmd = [self.deno_exe, "run"] + flags + [PERMISSIONS_PROMPT_TEST_TS
                                                ] + args
        return tty_capture(cmd, bytes_input)

    def warm_up(self):
        # ignore the ts compiling message
        self.run(["--allow-write"], 'needsWrite', b'')

    def test(self):
        for test_type in self.test_types:
            test_name_base = "test_" + test_type
            wrap_test(test_name_base + "_allow_flag", self.test_allow_flag,
                      test_type)
            wrap_test(test_name_base + "_yes_yes", self.test_yes_yes,
                      test_type)
            wrap_test(test_name_base + "_yes_no", self.test_yes_no, test_type)
            wrap_test(test_name_base + "_no_no", self.test_no_no, test_type)
            wrap_test(test_name_base + "_no_yes", self.test_no_yes, test_type)
            wrap_test(test_name_base + "_allow", self.test_allow, test_type)
            wrap_test(test_name_base + "_deny", self.test_deny, test_type)
            wrap_test(test_name_base + "_unrecognized_option",
                      self.test_unrecognized_option, test_type)
            wrap_test(test_name_base + "_no_prompt", self.test_no_prompt,
                      test_type)
            wrap_test(test_name_base + "_no_prompt_allow",
                      self.test_no_prompt_allow, test_type)

    def test_allow_flag(self, test_type):
        code, stdout, stderr = self.run(
            ["--allow-" + test_type], ["needs" + test_type.capitalize()], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_yes_yes(self, test_type):
        code, stdout, stderr = self.run([], ["needs" + test_type.capitalize()],
                                        b'y\ny\n')
        assert code == 0
        assert PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_yes_no(self, test_type):
        code, stdout, stderr = self.run([], ["needs" + test_type.capitalize()],
                                        b'y\nn\n')
        assert code == 1
        assert PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_no_no(self, test_type):
        code, stdout, stderr = self.run([], ["needs" + test_type.capitalize()],
                                        b'n\nn\n')
        assert code == 1
        assert PROMPT_PATTERN in stderr
        assert FIRST_CHECK_FAILED_PATTERN in stdout
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_no_yes(self, test_type):
        code, stdout, stderr = self.run([], ["needs" + test_type.capitalize()],
                                        b'n\ny\n')
        assert code == 0

        assert PROMPT_PATTERN in stderr
        assert FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_allow(self, test_type):
        code, stdout, stderr = self.run([], ["needs" + test_type.capitalize()],
                                        b'a\n')
        assert code == 0
        assert PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr

    def test_deny(self, test_type):
        code, stdout, stderr = self.run([], ["needs" + test_type.capitalize()],
                                        b'd\n')
        assert code == 1
        assert PROMPT_PATTERN in stderr
        assert FIRST_CHECK_FAILED_PATTERN in stdout
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_unrecognized_option(self, test_type):
        code, stdout, stderr = self.run([], ["needs" + test_type.capitalize()],
                                        b'e\na\n')
        assert code == 0
        assert PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr
        assert b'Unrecognized option' in stderr

    def test_no_prompt(self, test_type):
        code, stdout, stderr = self.run(
            ["--no-prompt"], ["needs" + test_type.capitalize()], b'')
        assert code == 1
        assert not PROMPT_PATTERN in stderr
        assert FIRST_CHECK_FAILED_PATTERN in stdout
        assert PERMISSION_DENIED_PATTERN in stderr

    def test_no_prompt_allow(self, test_type):
        code, stdout, stderr = self.run(
            ["--no-prompt", "--allow-" + test_type],
            ["needs" + test_type.capitalize()], b'')
        assert code == 0
        assert not PROMPT_PATTERN in stderr
        assert not FIRST_CHECK_FAILED_PATTERN in stdout
        assert not PERMISSION_DENIED_PATTERN in stderr


def permission_prompt_test(deno_exe):
    p = Prompt(deno_exe, ["read", "write", "env", "net", "run"])
    p.test()


def main():
    print "Permissions prompt tests"
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    permission_prompt_test(deno_exe)


if __name__ == "__main__":
    main()
