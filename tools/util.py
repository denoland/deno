# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import collections
import os
import re
import shutil
import select
import stat
import sys
import subprocess
import tempfile
import time

if os.environ.get("NO_COLOR", None):
    RESET = FG_READ = FG_GREEN = ""
else:
    RESET = "\x1b[0m"
    FG_RED = "\x1b[31m"
    FG_GREEN = "\x1b[32m"

executable_suffix = ".exe" if os.name == "nt" else ""

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
third_party_path = os.path.join(root_path, "third_party")


def make_env(merge_env=None, env=None):
    if env is None:
        env = os.environ
    env = env.copy()
    if merge_env is None:
        merge_env = {}
    for key in merge_env.keys():
        env[key] = merge_env[key]
    return env


def add_env_path(add, env, key="PATH", prepend=False):
    dirs_left = env[key].split(os.pathsep) if key in env else []
    dirs_right = add.split(os.pathsep) if isinstance(add, str) else add

    if prepend:
        dirs_left, dirs_right = dirs_right, dirs_left

    for d in dirs_right:
        if not d in dirs_left:
            dirs_left += [d]

    env[key] = os.pathsep.join(dirs_left)


def run(args, quiet=False, cwd=None, env=None, merge_env=None, shell=None):
    args[0] = os.path.normpath(args[0])
    env = make_env(env=env, merge_env=merge_env)
    if shell is None:
        # Use the default value for 'shell' parameter.
        #   - Posix: do not use shell.
        #   - Windows: use shell; this makes .bat/.cmd files work.
        shell = os.name == "nt"
    if not quiet:
        print(" ".join([shell_quote(arg) for arg in args]))
    rc = subprocess.call(args, cwd=cwd, env=env, shell=shell)
    if rc != 0:
        sys.exit(rc)


CmdResult = collections.namedtuple('CmdResult', ['out', 'err', 'code'])


def run_output(args,
               quiet=False,
               cwd=None,
               env=None,
               merge_env=None,
               exit_on_fail=False):
    if merge_env is None:
        merge_env = {}
    args[0] = os.path.normpath(args[0])
    if not quiet:
        print(" ".join(args))
    env = make_env(env=env, merge_env=merge_env)
    shell = os.name == "nt"  # Run through shell to make .bat/.cmd files work.
    p = subprocess.Popen(
        args,
        cwd=cwd,
        env=env,
        shell=shell,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE)
    try:
        out, err = p.communicate()
    except subprocess.CalledProcessError as e:
        p.kill()
        p.wait()
        raise e
    retcode = p.poll()
    if retcode and exit_on_fail:
        sys.exit(retcode)
    # Ignore Windows CRLF (\r\n).
    return CmdResult(
        out.replace('\r\n', '\n'), err.replace('\r\n', '\n'), retcode)


def shell_quote_win(arg):
    if re.search(r'[\x00-\x20"^%~!@&?*<>|()=]', arg):
        # Double all " quote characters.
        arg = arg.replace('"', '""')
        # Wrap the entire string in " quotes.
        arg = '"' + arg + '"'
        # Double any N backslashes that are immediately followed by a " quote.
        arg = re.sub(r'(\\+)(?=")', r'\1\1', arg)
    return arg


def shell_quote(arg):
    if os.name == "nt":
        return shell_quote_win(arg)
    else:
        # Python 2 has posix shell quoting built in, albeit in a weird place.
        from pipes import quote
        return quote(arg)


# Recursively list all files in (a subdirectory of) a git worktree.
#   * Optionally, glob patterns may be specified to e.g. only list files with a
#     certain extension.
#   * Untracked files are included, unless they're listed in .gitignore.
#   * Directory names themselves are not listed (but the files inside are).
#   * Submodules and their contents are ignored entirely.
#   * This function fails if the query matches no files.
def git_ls_files(base_dir, patterns=None):
    base_dir = os.path.abspath(base_dir)
    args = [
        "git", "-C", base_dir, "ls-files", "-z", "--exclude-standard",
        "--cached", "--modified", "--others"
    ]
    if patterns:
        args += ["--"] + patterns
    output = subprocess.check_output(args)
    files = [
        os.path.normpath(os.path.join(base_dir, f)) for f in output.split("\0")
        if f != ""
    ]
    if not files:
        raise RuntimeError("git_ls_files: no files in '%s'" % base_dir +
                           (" matching %s" % patterns if patterns else ""))
    return files


# list all files staged for commit
def git_staged(base_dir, patterns=None):
    base_dir = os.path.abspath(base_dir)
    args = [
        "git", "-C", base_dir, "diff", "--staged", "--diff-filter=ACMR",
        "--name-only", "-z"
    ]
    if patterns:
        args += ["--"] + patterns
    output = subprocess.check_output(args)
    files = [
        os.path.normpath(os.path.join(base_dir, f)) for f in output.split("\0")
        if f != ""
    ]
    return files


def build_mode():
    if "--release" in sys.argv:
        return "release"
    else:
        return "debug"


# E.G. "target/debug"
def build_path():
    return os.path.join(root_path, "target", build_mode())


# Attempts to enable ANSI escape code support.
# Returns True if successful, False if not supported.
def enable_ansi_colors():
    if os.name != 'nt':
        return True  # On non-windows platforms this just works.
    return enable_ansi_colors_win10()


# The windows 10 implementation of enable_ansi_colors.
def enable_ansi_colors_win10():
    import ctypes

    # Function factory for errcheck callbacks that raise WinError on failure.
    def raise_if(error_result):
        def check(result, _func, args):
            if result == error_result:
                raise ctypes.WinError(ctypes.get_last_error())
            return args

        return check

    # Windows API types.
    from ctypes.wintypes import BOOL, DWORD, HANDLE, LPCWSTR, LPVOID
    LPDWORD = ctypes.POINTER(DWORD)

    # Generic constants.
    NULL = ctypes.c_void_p(0).value
    INVALID_HANDLE_VALUE = ctypes.c_void_p(-1).value
    ERROR_INVALID_PARAMETER = 87

    # CreateFile flags.
    # yapf: disable
    GENERIC_READ  = 0x80000000
    GENERIC_WRITE = 0x40000000
    FILE_SHARE_READ  = 0x01
    FILE_SHARE_WRITE = 0x02
    OPEN_EXISTING = 3
    # yapf: enable

    # Get/SetConsoleMode flags.
    ENABLE_VIRTUAL_TERMINAL_PROCESSING = 0x04

    kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)

    # HANDLE CreateFileW(...)
    CreateFileW = kernel32.CreateFileW
    CreateFileW.restype = HANDLE
    CreateFileW.errcheck = raise_if(INVALID_HANDLE_VALUE)
    # yapf: disable
    CreateFileW.argtypes = (LPCWSTR,  # lpFileName
                            DWORD,    # dwDesiredAccess
                            DWORD,    # dwShareMode
                            LPVOID,   # lpSecurityAttributes
                            DWORD,    # dwCreationDisposition
                            DWORD,    # dwFlagsAndAttributes
                            HANDLE)   # hTemplateFile
    # yapf: enable

    # BOOL CloseHandle(HANDLE hObject)
    CloseHandle = kernel32.CloseHandle
    CloseHandle.restype = BOOL
    CloseHandle.errcheck = raise_if(False)
    CloseHandle.argtypes = (HANDLE, )

    # BOOL GetConsoleMode(HANDLE hConsoleHandle, LPDWORD lpMode)
    GetConsoleMode = kernel32.GetConsoleMode
    GetConsoleMode.restype = BOOL
    GetConsoleMode.errcheck = raise_if(False)
    GetConsoleMode.argtypes = (HANDLE, LPDWORD)

    # BOOL SetConsoleMode(HANDLE hConsoleHandle, DWORD dwMode)
    SetConsoleMode = kernel32.SetConsoleMode
    SetConsoleMode.restype = BOOL
    SetConsoleMode.errcheck = raise_if(False)
    SetConsoleMode.argtypes = (HANDLE, DWORD)

    # Open the console output device.
    conout = CreateFileW("CONOUT$", GENERIC_READ | GENERIC_WRITE,
                         FILE_SHARE_READ | FILE_SHARE_WRITE, NULL,
                         OPEN_EXISTING, 0, 0)

    # Get the current mode.
    mode = DWORD()
    GetConsoleMode(conout, ctypes.byref(mode))

    # Try to set the flag that controls ANSI escape code support.
    try:
        SetConsoleMode(conout, mode.value | ENABLE_VIRTUAL_TERMINAL_PROCESSING)
    except WindowsError as e:  # pylint:disable=undefined-variable
        if e.winerror == ERROR_INVALID_PARAMETER:
            return False  # Not supported, likely an older version of Windows.
        raise
    finally:
        CloseHandle(conout)

    return True


def print_command(cmd, files):
    noun = "file" if len(files) == 1 else "files"
    print("%s (%d %s)" % (cmd, len(files), noun))
