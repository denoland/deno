#!/usr/bin/env python
# Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
# This script contains helper functions to work with the third_party subrepo.

import os
import re
import site
import sys
from tempfile import mkdtemp
from util import add_env_path, executable_suffix, make_env, rmtree
from util import root_path, run, third_party_path

depot_tools_path = os.path.join(third_party_path, "depot_tools")
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


# Run Yarn to install JavaScript dependencies.
def run_yarn():
    node_modules_path = os.path.join(third_party_path, "node_modules")
    # Note to keep the root directory clean, we keep package.json is in tools/.
    run([
        "yarn", "install", "--no-lockfile",
        "--modules-folder=" + node_modules_path
    ],
        cwd=os.path.join(root_path, "tools"))


# Install python packages with pip.
def run_pip():
    # Install an recent version of pip into a temporary directory. The version
    # that is bundled with python is too old to support the next step.
    temp_python_home = mkdtemp()
    pip_env = {"PYTHONUSERBASE": temp_python_home}
    run([sys.executable, "-m", "pip", "install", "--upgrade", "--user", "pip"],
        cwd=third_party_path,
        merge_env=pip_env)

    # Install pywin32.
    run([
        sys.executable, "-m", "pip", "install", "--upgrade", "--target",
        python_packages_path, "--platform=win_amd64", "--only-binary=:all:",
        "pypiwin32"
    ],
        cwd=third_party_path,
        merge_env=pip_env)

    # Get yapf.
    run([
        sys.executable, "-m", "pip", "install", "--upgrade", "--target",
        python_packages_path, "yapf"
    ],
        cwd=third_party_path,
        merge_env=pip_env)

    run([
        sys.executable, "-m", "pip", "install", "--upgrade", "--target",
        python_packages_path, "pylint==1.5.6"
    ],
        cwd=third_party_path,
        merge_env=pip_env)

    # Remove the temporary pip installation.
    rmtree(temp_python_home)


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
