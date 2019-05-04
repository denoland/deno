#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import pty
import select
import subprocess
from util import build_path, executable_suffix
from sys import stdin
from permission_prompt_test import tty_capture

IS_TTY_TEST_TS = "tests/is_tty.ts"


def is_tty_test(deno_exe):
    cmd = [deno_exe, "run", IS_TTY_TEST_TS]
    code, stdout, _ = tty_capture(cmd, b'')
    assert code == 0
    assert str(stdin.isatty()).lower() in stdout


def main():
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    is_tty_test(deno_exe)


if __name__ == "__main__":
    main()
