# Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
# All rights reserved. MIT License.
import os
import subprocess


def run(args):
    print " ".join(args)
    env = os.environ.copy()
    if os.name == "nt":
        # Run through shell to make .bat/.cmd files work.
        args = ["cmd", "/c"] + args
    subprocess.check_call(args, env=env)


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
        import ctypes
        CreateSymbolicLinkW = ctypes.windll.kernel32.CreateSymbolicLinkW
        CreateSymbolicLinkW.restype = ctypes.c_ubyte
        CreateSymbolicLinkW.argtypes = (ctypes.c_wchar_p, ctypes.c_wchar_p,
                                        ctypes.c_uint32)

        flags = 0x02  # SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE
        if (target_is_dir):
            flags |= 0x01  # SYMBOLIC_LINK_FLAG_DIRECTORY
        if not CreateSymbolicLinkW(name, target, flags):
            raise ctypes.WinError()
    else:
        os.symlink(target, name)


def touch(fname):
    if os.path.exists(fname):
        os.utime(fname, None)
    else:
        open(fname, 'a').close()
