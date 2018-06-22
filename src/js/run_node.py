#!/usr/bin/env python
"""
gn can only run python scripts. This launches a subprocess Node process.
The working dir of this program is out/Debug/ (AKA root_build_dir)
Before running node, we symlink js/node_modules to out/Debug/node_modules.
"""
import subprocess
import sys
import os

def symlink(target, name, target_is_dir=False):
  if os.name == "nt":
    import ctypes
    CreateSymbolicLinkW = ctypes.windll.kernel32.CreateSymbolicLinkW
    CreateSymbolicLinkW.restype = ctypes.c_ubyte
    CreateSymbolicLinkW.argtypes = (ctypes.c_wchar_p,
                                    ctypes.c_wchar_p,
                                    ctypes.c_uint32)

    flags = 0x02  # SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE
    if (target_is_dir):
      flags |= 0x01  # SYMBOLIC_LINK_FLAG_DIRECTORY
    if not CreateSymbolicLinkW(name, target, flags):
      raise ctypes.WinError()
  else:
    os.symlink(target, name)

js_path = os.path.dirname(os.path.realpath(__file__))
node_modules_path = os.path.join(js_path, "node_modules")

if not os.path.lexists("node_modules"):
  if os.path.exists("node_modules"):
    os.unlink("node_modules")
  symlink(node_modules_path, "node_modules", True)

args = ["node"] + sys.argv[1:]
sys.exit(subprocess.call(args))
