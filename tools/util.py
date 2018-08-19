# Copyright 2018 the Deno authors. All rights reserved. MIT license.
import os
import re
import shutil
import stat
import sys
import subprocess

executable_suffix = ".exe" if os.name == "nt" else ""
root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))


def make_env(merge_env={}, env=None):
    if env is None:
        env = os.environ
    env = env.copy()
    for key in merge_env.keys():
        env[key] = merge_env[key]
    return env


def run(args, quiet=False, cwd=None, env=None, merge_env={}):
    args[0] = os.path.normpath(args[0])
    if not quiet:
        print " ".join(args)
    env = make_env(env=env, merge_env=merge_env)
    shell = os.name == "nt"  # Run through shell to make .bat/.cmd files work.
    rc = subprocess.call(args, cwd=cwd, env=env, shell=shell)
    if rc != 0:
        sys.exit(rc)


def run_output(args, quiet=False, cwd=None, env=None, merge_env={}):
    args[0] = os.path.normpath(args[0])
    if not quiet:
        print " ".join(args)
    env = make_env(env=env, merge_env=merge_env)
    shell = os.name == "nt"  # Run through shell to make .bat/.cmd files work.
    return subprocess.check_output(args, cwd=cwd, env=env, shell=shell)


def remove_and_symlink(target, name, target_is_dir=False):
    try:
        # On Windows, directory symlink can only be removed with rmdir().
        if os.name == "nt" and os.path.isdir(name):
            os.rmdir(name)
        else:
            os.unlink(name)
    except:
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
# (Recursive glob doesn't exist in python 2.7.)
def find_exts(directory, *extensions):
    matches = []
    for root, dirnames, filenames in os.walk(directory):
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


# E.G. "out/debug"
def build_path():
    if "DENO_BUILD_PATH" in os.environ:
        return os.environ["DENO_BUILD_PATH"]
    else:
        return os.path.join(root_path, "out", build_mode())


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
