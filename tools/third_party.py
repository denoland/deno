#!/usr/bin/env python
# Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
# This script contains helper functions to work with the third_party subrepo.

import os
import re
import site
import sys
from tempfile import mkdtemp
from util import add_env_path, executable_suffix, libdeno_path, make_env, rmtree
from util import root_path, run, third_party_path

depot_tools_path = os.path.join(third_party_path, "depot_tools")
prebuilt_path = os.path.join(root_path, "prebuilt")
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
        site.addsitedir(os.path.join(libdeno_path,
                                     "build"))  # Modifies PATH and sys.path.
        site.addsitedir(python_packages_path)  # Modifies PATH and sys.path.
        python_site_env = {"PATH": os.environ["PATH"], "PYTHONPATH": sys.path}
        os.environ["PATH"], sys.path = temp

    # Make a new environment object.
    env = make_env(env=env, merge_env=merge_env)
    # Apply PATH and PYTHONPATH from the site-packages environment.
    add_env_path(python_site_env["PATH"], env=env, key="PATH")
    add_env_path(python_site_env["PYTHONPATH"], env=env, key="PYTHONPATH")

    return env


# This function creates or modifies an environment so that it matches the
# expectations of various google tools (gn, gclient, etc).
def google_env(env=None, merge_env=None, depot_tools_path_=depot_tools_path):
    if merge_env is None:
        merge_env = {}
    # Google tools need the python env too.
    env = python_env(env=env, merge_env=merge_env)

    # Depot_tools to be in the PATH, before Python.
    add_env_path(depot_tools_path_, env=env, prepend=True)

    if os.name == "nt":  # Windows-only enviroment tweaks.
        # We're not using Google's internal infrastructure.
        if os.name == "nt" and not "DEPOT_TOOLS_WIN_TOOLCHAIN" in env:
            env["DEPOT_TOOLS_WIN_TOOLCHAIN"] = "0"

        # The 'setup_toolchain.py' script does a good job finding the Windows
        # SDK. Unfortunately, if any of the environment variables below are set
        # (as vcvarsall.bat typically would), setup_toolchain absorbs them too,
        # adding multiple identical -imsvc<path> items to CFLAGS.
        # This small variation has no effect on compiler output, but it
        # makes ninja rebuild everything, and causes sccache cache misses.
        # TODO(piscisaureus): fix this upstream.
        env["INCLUDE"] = ""
        env["LIB"] = ""
        env["LIBPATH"] = ""

    return env


# Run Yarn to install JavaScript dependencies.
def run_yarn():
    run(["yarn", "install"], cwd=third_party_path)


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


# Run gclient to install other dependencies.
def run_gclient_sync():
    # Depot_tools will normally try to self-update, which will fail because
    # it's not checked out from it's own git repository; gclient will then try
    # to fix things up and not succeed, and and we'll end up with a huge mess.
    # To work around this, we rename the `depot_tools` directory to
    # `{root_path}/depot_tools_temp` first, and we set DEPOT_TOOLS_UPDATE=0 in
    # the environment so depot_tools doesn't attempt to self-update.
    # Since depot_tools is listed in .gclient_entries, gclient will install a
    # fresh copy in `third_party/depot_tools`.
    # If it all works out, we remove the depot_tools_temp directory afterwards.
    depot_tools_temp_path = os.path.join(root_path, "depot_tools_temp")

    # Rename depot_tools to depot_tools_temp.
    try:
        os.rename(depot_tools_path, depot_tools_temp_path)
    except OSError:
        # If renaming failed, and the depot_tools_temp directory already exists,
        # assume that it's still there because a prior run_gclient_sync() call
        # failed half-way, before we got the chance to remove the temp dir.
        # We'll use whatever is in the temp dir that was already there.
        # If not, the user can recover by removing the temp directory manually.
        if os.path.isdir(depot_tools_temp_path):
            pass
        else:
            raise

    args = [
        "gclient", "sync", "--reset", "--shallow", "--no-history", "--nohooks"
    ]
    envs = {
        "DEPOT_TOOLS_UPDATE": "0",
        "GCLIENT_FILE": os.path.join(root_path, "gclient_config.py")
    }
    env = google_env(depot_tools_path_=depot_tools_temp_path, merge_env=envs)
    run(args, cwd=third_party_path, env=env)

    # Delete the depot_tools_temp directory, but not before verifying that
    # gclient did indeed install a fresh copy.
    # Also check that `{depot_tools_temp_path}/gclient.py` exists, so a typo in
    # this script won't accidentally blow out someone's home dir.
    if (os.path.isdir(os.path.join(depot_tools_path, ".git"))
            and os.path.isfile(os.path.join(depot_tools_path, "gclient.py"))
            and os.path.isfile(
                os.path.join(depot_tools_temp_path, "gclient.py"))):
        rmtree(depot_tools_temp_path)


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


def get_buildtools_tool_path(tool):
    return os.path.join(libdeno_path, "buildtools", get_platform_dir_name(),
                        tool + executable_suffix)


# Download the given item from Google storage.
def download_from_google_storage(item, bucket, base_dir):
    download_script = os.path.join(depot_tools_path,
                                   "download_from_google_storage.py")
    sha1_file = os.path.join(base_dir, get_platform_dir_name(),
                             item + executable_suffix + ".sha1")
    run([
        sys.executable,
        download_script,
        "--platform=" + sys.platform,
        "--no_auth",
        "--bucket=%s" % bucket,
        "--sha1_file",
        sha1_file,
    ],
        env=google_env())


# Download the given item from Chrome Infrastructure Package Deployment.
def download_from_cipd(item, version):
    cipd_exe = os.path.join(depot_tools_path, "cipd")
    download_dir = os.path.join(libdeno_path, "buildtools",
                                get_platform_dir_name())

    if sys.platform == "win32":
        item += "windows-amd64"
    elif sys.platform == "darwin":
        item += "mac-amd64"
    elif sys.platform.startswith("linux"):
        item += "linux-amd64"

    # Init cipd if necessary.
    if not os.path.exists(os.path.join(download_dir, ".cipd")):
        run([
            cipd_exe,
            "init",
            download_dir,
            "-force",
        ], env=google_env())

    run([
        cipd_exe,
        "install",
        item,
        "git_revision:" + version,
        "-root",
        download_dir,
    ],
        env=google_env())


# Download gn from Google storage.
def download_gn():
    download_from_cipd("gn/gn/", "152c5144ceed9592c20f0c8fd55769646077569b")


# Download clang-format from Google storage.
def download_clang_format():
    download_from_google_storage("clang-format", "chromium-clang-format",
                                 os.path.join(libdeno_path, "buildtools"))


def download_sccache():
    download_from_google_storage("sccache", "denoland", prebuilt_path)


def download_hyperfine():
    download_from_google_storage("hyperfine", "denoland", prebuilt_path)


# Download clang by calling the clang update script.
def download_clang():
    update_script = os.path.join(libdeno_path, "v8", "tools", "clang",
                                 "scripts", "update.py")
    run([sys.executable, update_script], env=google_env())


def maybe_download_sysroot():
    if sys.platform.startswith("linux"):
        install_script = os.path.join(libdeno_path, "build", "linux",
                                      "sysroot_scripts", "install-sysroot.py")
        run([sys.executable, install_script, "--arch=amd64"], env=google_env())
