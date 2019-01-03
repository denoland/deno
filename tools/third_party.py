#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# This script contains helper functions to work with the third_party subrepo.

import os
import site
import sys
from os import path
from util import add_env_path, find_exts, make_env, remove_and_symlink, rmtree
from util import root_path, run
from tempfile import mkdtemp


# Helper function that returns the full path to a subpath of the repo root.
def root(*subpath_parts):
    return path.normpath(path.join(root_path, *subpath_parts))


# Helper function that returns the full path to a file/dir in third_party.
def tp(*subpath_parts):
    return root("third_party", *subpath_parts)


third_party_path = tp()
depot_tools_path = tp("depot_tools")
rust_crates_path = tp("rust_crates")
python_packages_path = tp("python_packages")
gn_path = tp(depot_tools_path, "gn")
clang_format_path = tp(depot_tools_path, "clang-format")
ninja_path = tp(depot_tools_path, "ninja")

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


def fix_symlinks():
    # Ensure the third_party directory exists.
    try:
        os.makedirs(third_party_path)
    except OSError:
        pass

    # Make symlinks to Yarn metadata living in the root repo.
    remove_and_symlink("../package.json", tp("package.json"))

    # TODO(ry) Is it possible to remove these symlinks?
    remove_and_symlink("v8/third_party/googletest", tp("googletest"), True)
    remove_and_symlink("v8/third_party/jinja2", tp("jinja2"), True)
    remove_and_symlink("v8/third_party/llvm-build", tp("llvm-build"), True)
    remove_and_symlink("v8/third_party/markupsafe", tp("markupsafe"), True)
    remove_and_symlink("../../build", tp("v8/build"), True)

    # On Windows, git doesn't create the right type of symlink if the symlink
    # and it's target are in different repos. Here we fix the symlinks that
    # exist in the root repo while their target is in the third_party repo.
    remove_and_symlink("third_party/node_modules", root("node_modules"), True)
    remove_and_symlink("third_party/v8/buildtools", root("buildtools"), True)
    remove_and_symlink("third_party/v8/build_overrides",
                       root("build_overrides"), True)
    remove_and_symlink("third_party/v8/testing", root("testing"), True)
    remove_and_symlink("../third_party/v8/tools/clang", root("tools/clang"),
                       True)


# Run Yarn to install JavaScript dependencies.
def run_yarn():
    run(["yarn", "install"], cwd=third_party_path)


# Run Cargo to install Rust dependencies.
def run_cargo():
    # Deletes the cargo index lockfile; it appears that cargo itself doesn't do
    # it.  If the lockfile ends up in the git repo, it'll make cargo hang for
    # everyone else who tries to run sync_third_party.
    def delete_lockfile():
        lockfiles = find_exts([path.join(rust_crates_path, "registry/index")],
                              ['.cargo-index-lock'])
        for lockfile in lockfiles:
            os.remove(lockfile)

    # Delete the index lockfile in case someone accidentally checked it in.
    delete_lockfile()

    run(["cargo", "fetch", "--manifest-path=" + root("Cargo.toml")],
        cwd=third_party_path,
        merge_env={'CARGO_HOME': rust_crates_path})

    # Delete the lockfile again so it doesn't end up in the git repo.
    delete_lockfile()


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
    depot_tools_temp_path = root("depot_tools_temp")

    # Rename depot_tools to depot_tools_temp.
    try:
        os.rename(depot_tools_path, depot_tools_temp_path)
    except OSError:
        # If renaming failed, and the depot_tools_temp directory already exists,
        # assume that it's still there because a prior run_gclient_sync() call
        # failed half-way, before we got the chance to remove the temp dir.
        # We'll use whatever is in the temp dir that was already there.
        # If not, the user can recover by removing the temp directory manually.
        if path.isdir(depot_tools_temp_path):
            pass
        else:
            raise

    args = [
        "gclient", "sync", "--reset", "--shallow", "--no-history", "--nohooks"
    ]
    envs = {
        'DEPOT_TOOLS_UPDATE': "0",
        'GCLIENT_FILE': root("gclient_config.py")
    }
    env = google_env(depot_tools_path_=depot_tools_temp_path, merge_env=envs)
    run(args, cwd=third_party_path, env=env)

    # Delete the depot_tools_temp directory, but not before verifying that
    # gclient did indeed install a fresh copy.
    # Also check that `{depot_tools_temp_path}/gclient.py` exists, so a typo in
    # this script won't accidentally blow out someone's home dir.
    if (path.isdir(path.join(depot_tools_path, ".git"))
            and path.isfile(path.join(depot_tools_path, "gclient.py"))
            and path.isfile(path.join(depot_tools_temp_path, "gclient.py"))):
        rmtree(depot_tools_temp_path)


# Download the given item from Google storage.
def download_from_google_storage(item, bucket):
    if sys.platform == 'win32':
        sha1_file = "v8/buildtools/win/%s.exe.sha1" % item
    elif sys.platform == 'darwin':
        sha1_file = "v8/buildtools/mac/%s.sha1" % item
    elif sys.platform.startswith('linux'):
        sha1_file = "v8/buildtools/linux64/%s.sha1" % item

    run([
        "python",
        tp('depot_tools/download_from_google_storage.py'),
        '--platform=' + sys.platform,
        '--no_auth',
        '--bucket=%s' % bucket,
        '--sha1_file',
        tp(sha1_file),
    ],
        env=google_env())


# Download gn from Google storage.
def download_gn():
    download_from_google_storage('gn', 'chromium-gn')


# Download clang-format from Google storage.
def download_clang_format():
    download_from_google_storage('clang-format', 'chromium-clang-format')


# Download clang by calling the clang update script.
def download_clang():
    run(['python',
         tp('v8/tools/clang/scripts/update.py'), '--if-needed'],
        env=google_env())


def maybe_download_sysroot():
    if sys.platform.startswith('linux'):
        run([
            'python',
            os.path.join(root_path,
                         'build/linux/sysroot_scripts/install-sysroot.py'),
            '--arch=amd64'
        ],
            env=google_env())
