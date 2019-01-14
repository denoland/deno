# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import re
import shutil
import stat
import sys
import subprocess

RESET = "\x1b[0m"
FG_RED = "\x1b[31m"
FG_GREEN = "\x1b[32m"

executable_suffix = ".exe" if os.name == "nt" else ""
root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
tests_path = os.path.join(root_path, "tests")


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


def run(args, quiet=False, cwd=None, env=None, merge_env=None):
    if merge_env is None:
        merge_env = {}
    args[0] = os.path.normpath(args[0])
    if not quiet:
        print " ".join(args)
    env = make_env(env=env, merge_env=merge_env)
    shell = os.name == "nt"  # Run through shell to make .bat/.cmd files work.
    rc = subprocess.call(args, cwd=cwd, env=env, shell=shell)
    if rc != 0:
        sys.exit(rc)


def run_output(args, quiet=False, cwd=None, env=None, merge_env=None):
    if merge_env is None:
        merge_env = {}
    args[0] = os.path.normpath(args[0])
    if not quiet:
        print " ".join(args)
    env = make_env(env=env, merge_env=merge_env)
    shell = os.name == "nt"  # Run through shell to make .bat/.cmd files work.
    return subprocess.check_output(args, cwd=cwd, env=env, shell=shell)


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


def red_failed():
    return "%sFAILED%s" % (FG_RED, RESET)


def green_ok():
    return "%sok%s" % (FG_GREEN, RESET)


def remove_and_symlink(target, name, target_is_dir=False):
    if os.name != "nt" and os.path.islink(name):
        return
    try:
        # On Windows, directory symlink can only be removed with rmdir().
        if os.name == "nt" and os.path.isdir(name):
            os.rmdir(name)
        else:
            os.unlink(name)
    except OSError:
        pass
    symlink(target, name, target_is_dir)


def symlink(target, name, target_is_dir=False):
    if os.name == "nt":
        from ctypes import WinDLL, WinError, GetLastError
        from ctypes.wintypes import BOOLEAN, DWORD, LPCWSTR

        kernel32 = WinDLL('kernel32', use_last_error=False)
        CreateSymbolicLinkW = kernel32.CreateSymbolicLinkW
        CreateSymbolicLinkW.restype = BOOLEAN
        CreateSymbolicLinkW.argtypes = (LPCWSTR, LPCWSTR, DWORD)

        # File-type symlinks can only use backslashes as separators.
        target = os.path.normpath(target)

        # If the symlink points at a directory, it needs to have the appropriate
        # flag set, otherwise the link will be created but it won't work.
        if target_is_dir:
            type_flag = 0x01  # SYMBOLIC_LINK_FLAG_DIRECTORY
        else:
            type_flag = 0

        # Before Windows 10, creating symlinks requires admin privileges.
        # As of Win 10, there is a flag that allows anyone to create them.
        # Initially, try to use this flag.
        unpriv_flag = 0x02  # SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE
        r = CreateSymbolicLinkW(name, target, type_flag | unpriv_flag)

        # If it failed with ERROR_INVALID_PARAMETER, try again without the
        # 'allow unprivileged create' flag.
        if not r and GetLastError() == 87:  # ERROR_INVALID_PARAMETER
            r = CreateSymbolicLinkW(name, target, type_flag)

        # Throw if unsuccessful even after the second attempt.
        if not r:
            raise WinError()
    else:
        os.symlink(target, name)


def touch(fname):
    if os.path.exists(fname):
        os.utime(fname, None)
    else:
        open(fname, 'a').close()


# Recursive search for files of certain extensions.
#   * Recursive glob doesn't exist in python 2.7.
#   * On windows, `os.walk()` unconditionally follows symlinks.
#     The `skip`  parameter should be used to avoid recursing through those.
def find_exts(directories, extensions, skip=None):
    if skip is None:
        skip = []
    assert isinstance(directories, list)
    assert isinstance(extensions, list)
    skip = [os.path.normpath(i) for i in skip]
    matches = []
    for directory in directories:
        for root, dirnames, filenames in os.walk(directory):
            if root in skip:
                dirnames[:] = []  # Don't recurse further into this directory.
                continue
            for filename in filenames:
                for ext in extensions:
                    if filename.endswith(ext):
                        matches.append(os.path.join(root, filename))
                        break
    return matches


# The Python equivalent of `rm -rf`.
def rmtree(directory):
    # On Windows, shutil.rmtree() won't delete files that have a readonly bit.
    # Git creates some files that do. The 'onerror' callback deals with those.
    def rm_readonly(func, path, _):
        os.chmod(path, stat.S_IWRITE)
        func(path)

    shutil.rmtree(directory, onerror=rm_readonly)


def build_mode(default="debug"):
    if "DENO_BUILD_MODE" in os.environ:
        return os.environ["DENO_BUILD_MODE"]
    else:
        return default


# E.G. "target/debug"
def build_path():
    if "DENO_BUILD_PATH" in os.environ:
        return os.environ["DENO_BUILD_PATH"]
    else:
        return os.path.join(root_path, "target", build_mode())


# Returns True if the expected matches the actual output, allowing variation
# from actual where expected has the wildcard (e.g. matches /.*/)
def pattern_match(pattern, string, wildcard="[WILDCARD]"):
    if len(pattern) == 0:
        return string == 0
    if pattern == wildcard:
        return True

    parts = str.split(pattern, wildcard)

    if len(parts) == 1:
        return pattern == string

    if string.startswith(parts[0]):
        string = string[len(parts[0]):]
    else:
        return False

    for i in range(1, len(parts)):
        if i == (len(parts) - 1):
            if parts[i] == "" or parts[i] == "\n":
                return True
        found = string.find(parts[i])
        if found < 0:
            return False
        string = string[(found + len(parts[i])):]

    return len(string) == 0


def parse_exit_code(s):
    codes = [int(d or 1) for d in re.findall(r'error(\d*)', s)]
    if len(codes) > 1:
        assert False, "doesn't support multiple error codes."
    elif len(codes) == 1:
        return codes[0]
    else:
        return 0


# Attempts to enable ANSI escape code support.
# Returns True if successful, False if not supported.
def enable_ansi_colors():
    if os.name != 'nt':
        return True  # On non-windows platforms this just works.
    elif "CI" in os.environ:
        return True  # Ansi escape codes work out of the box on Appveyor.

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


def parse_unit_test_output(output, print_to_stdout):
    expected = None
    actual = None
    result = None
    for line in iter(output.readline, ''):
        if expected is None:
            # expect "running 30 tests"
            expected = extract_number(r'running (\d+) tests', line)
        elif "test result:" in line:
            result = line
        if print_to_stdout:
            sys.stdout.write(line)
            sys.stdout.flush()
    # Check that the number of expected tests equals what was reported at the
    # bottom.
    if result:
        # result should be a string like this:
        # "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ..."
        actual = extract_number(r'(\d+) passed', result)
    return (actual, expected)


def extract_number(pattern, string):
    matches = re.findall(pattern, string)
    if len(matches) != 1:
        return None
    return int(matches[0])


def parse_wrk_output(output):
    req_per_sec = None
    for line in output.split("\n"):
        if req_per_sec is None:
            req_per_sec = extract_number(r'Requests/sec:\s+(\d+)', line)
    return req_per_sec


def platform():
    return {"linux2": "linux", "darwin": "mac", "win32": "win"}[sys.platform]
