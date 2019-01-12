#!/usr/bin/env python
import os
import pty
import select
import subprocess

from util import build_path, executable_suffix

PERMISSIONS_PROMPT_TEST_TS = "tools/permission_prompt_test.ts"


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


class Prompt(object):
    def __init__(self, deno_exe):
        self.deno_exe = deno_exe

    def run(self,
            arg,
            bytes_input,
            allow_write=False,
            allow_net=False,
            allow_env=False,
            allow_run=False):
        "Returns (return_code, stdout, stderr)."
        cmd = [self.deno_exe, PERMISSIONS_PROMPT_TEST_TS, arg]
        if allow_write:
            cmd.append("--allow-write")
        if allow_net:
            cmd.append("--allow-net")
        if allow_env:
            cmd.append("--allow-env")
        if allow_run:
            cmd.append("--allow-run")
        return tty_capture(cmd, bytes_input)

    def warm_up(self):
        # ignore the ts compiling message
        self.run('needsWrite', b'', allow_write=True)

    def test_write_yes(self):
        code, stdout, stderr = self.run('needsWrite', b'y\n')
        assert code == 0
        assert stdout == b''
        assert b'Deno requests write access' in stderr

    def test_write_arg(self):
        code, stdout, stderr = self.run('needsWrite', b'', allow_write=True)
        assert code == 0
        assert stdout == b''
        assert stderr == b''

    def test_write_no(self):
        code, _stdout, stderr = self.run('needsWrite', b'N\n')
        assert code == 1
        assert b'PermissionDenied: permission denied' in stderr
        assert b'Deno requests write access' in stderr

    def test_env_yes(self):
        code, stdout, stderr = self.run('needsEnv', b'y\n')
        assert code == 0
        assert stdout == b''
        assert b'Deno requests access to environment' in stderr

    def test_env_arg(self):
        code, stdout, stderr = self.run('needsEnv', b'', allow_env=True)
        assert code == 0
        assert stdout == b''
        assert stderr == b''

    def test_env_no(self):
        code, _stdout, stderr = self.run('needsEnv', b'N\n')
        assert code == 1
        assert b'PermissionDenied: permission denied' in stderr
        assert b'Deno requests access to environment' in stderr

    def test_net_yes(self):
        code, stdout, stderr = self.run('needsEnv', b'y\n')
        assert code == 0
        assert stdout == b''
        assert b'Deno requests access to environment' in stderr

    def test_net_arg(self):
        code, stdout, stderr = self.run('needsNet', b'', allow_net=True)
        assert code == 0
        assert stdout == b''
        assert stderr == b''

    def test_net_no(self):
        code, _stdout, stderr = self.run('needsNet', b'N\n')
        assert code == 1
        assert b'PermissionDenied: permission denied' in stderr
        assert b'Deno requests network access' in stderr

    def test_run_yes(self):
        code, stdout, stderr = self.run('needsRun', b'y\n')
        assert code == 0
        assert stdout == b'hello'
        assert b'Deno requests access to run' in stderr

    def test_run_arg(self):
        code, stdout, stderr = self.run('needsRun', b'', allow_run=True)
        assert code == 0
        assert stdout == b'hello'
        assert stderr == b''

    def test_run_no(self):
        code, _stdout, stderr = self.run('needsRun', b'N\n')
        assert code == 1
        assert b'PermissionDenied: permission denied' in stderr
        assert b'Deno requests access to run' in stderr


def permission_prompt_test(deno_exe):
    p = Prompt(deno_exe)
    p.warm_up()
    p.test_write_yes()
    p.test_write_arg()
    p.test_write_no()
    p.test_env_yes()
    p.test_env_arg()
    p.test_env_no()
    p.test_net_yes()
    p.test_net_arg()
    p.test_net_no()
    p.test_run_yes()
    p.test_run_arg()
    p.test_run_no()


def main():
    deno_exe = os.path.join(build_path(), "deno" + executable_suffix)
    permission_prompt_test(deno_exe)


if __name__ == "__main__":
    main()
