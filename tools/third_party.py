#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# This script contains helper functions to work with the third_party subrepo.

import os
import re
import site
import sys
from util import add_env_path, executable_suffix, make_env, third_party_path

prebuilt_path = os.path.join(third_party_path, "prebuilt")
python_packages_path = os.path.join(third_party_path, "python_packages")

python_site_env = None


# Creates/modifies an environment so python can find packages that are bundled
# in the 'third_party' directory.
def python_env(env=None, merge_env=None):
    if merge_env is None:
        merge_env = {}
    global python_site_env

    # Use site.addsitedir() to determine which search paths would be considered
    # if 'third_party/python_packages' was a site-packages directory.
    # PATH is also updated, so windows can find the DLLs that ship with pywin32.
    if python_site_env is None:
        python_site_env = {}
        temp = os.environ["PATH"], sys.path
        os.environ["PATH"], sys.path = "", []
        site.addsitedir(python_packages_path)  # Modifies PATH and sys.path.
        python_site_env = {"PATH": os.environ["PATH"], "PYTHONPATH": sys.path}
        os.environ["PATH"], sys.path = temp

    # Make a new environment object.
    env = make_env(env=env, merge_env=merge_env)
    # Apply PATH and PYTHONPATH from the site-packages environment.
    add_env_path(python_site_env["PATH"], env=env, key="PATH")
    add_env_path(python_site_env["PYTHONPATH"], env=env, key="PYTHONPATH")

    return env


def get_platform_dir_name():
    if sys.platform == "win32":
        return "win"
    elif sys.platform == "darwin":
        return "mac"
    elif sys.platform.startswith("linux"):
        return "linux64"


def get_prebuilt_tool_path(tool):
    return os.path.join(prebuilt_path, get_platform_dir_name(),
                        tool + executable_suffix)
