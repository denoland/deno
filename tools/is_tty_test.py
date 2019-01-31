#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import os
import pty
import select
import subprocess

from util import build_path, executable_suffix

from sys import stdin

IS_TTY_TEST_TS = "tools/is_tty_test.ts"


# This function is copied from:
# https://gist.github.com/hayd/4f46a68fc697ba8888a7b517a414583e
# https://stackoverflow.com/q/52954248/1240268
def tty_capture(cmd, bytes_input):
    """Capture the output of cmd with bytes_input to stdin,
    with stdin, stdout and stderr as TTYs."""
    mo, so = pty.openpty()  # provide tty to enable line-buffering
    me, se = pty.openpty()
    mi, si = pty.openpty()
    fdmap = {mo: 'stdout', me: 'stderr', mi: 'stdin'}

    p = subprocess.Popen(
        cmd, bufsize=1, stdin=si, stdout=so, stderr=se, close_fds=True)
    os.write(mi, bytes_input)

    timeout = .04  # seconds
    res = {'stdout': b'', 'stderr': b''}
    while True:
        ready, _, _ = select.select([mo, me], [], [], timeout)
        if ready:
            for fd in ready:
                data = os.read(fd, 512)
                if not data:
                    break
                res[fdmap[fd]] += data
        elif p.poll() is not None:  # select timed-out
            break  # p exited
    for fd in [si, so, se, mi, mo, me]:
        os.close(fd)  # can't do it sooner: it leads to errno.EIO error
    p.wait()
    return p.returncode, res['stdout'], res['stderr']


class IsTTY(object):
    def __init__(self, deno_exe):
        self.deno_exe = deno_exe

    def run(self,
            arg):
        "Returns (return_code, stdout, stderr)."
        cmd = [self.deno_exe, IS_TTY_TEST_TS, arg]
        return tty_capture(cmd, b'')

    def test_stdin(self):
        code, stdout, _ = self.run('stdin')
        assert code == 0
        assert str(stdin.isatty()).lower() in stdout

    def test_stdout(self):
        code, stdout, _ = self.run('stdout')
        assert code == 0
        assert str(stdin.isatty()).lower() in stdout

    def test_stderr(self):
        code, stdout, _ = self.run('stderr')
        assert code == 0
        assert str(stdin.isatty()).lower() in stdout

    def test_file(self):
        code, stdout, _ = self.run('file')
        assert code == 0
        assert b'false' in stdout


def is_tty_test(deno_exe):
    p = IsTTY(deno_exe)
    p.test_stdin()
    p.test_stdout()
    p.test_stderr()
    p.test_file()


def main():
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    is_tty_test(deno_exe)


if __name__ == "__main__":
    main()
