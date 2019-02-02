#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import pty
import select
import subprocess

from util import build_path, executable_suffix

from sys import stdin

IS_TTY_TEST_TS = "tests/is_tty.ts"

from permission_prompt_test import tty_capture

class IsTTY(object):
    def __init__(self, deno_exe):
        self.deno_exe = deno_exe

    def test(self):
        cmd = [self.deno_exe, IS_TTY_TEST_TS, b'']
        code, stdout, _ = tty_capture(cmd, b'')
        assert code == 0
        assert str(stdin.isatty()).lower() in stdout

def is_tty_test(deno_exe):
    p = IsTTY(deno_exe)
    p.test()

def main():
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    is_tty_test(deno_exe)

if __name__ == "__main__":
    main()
