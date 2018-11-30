#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
#
# The Rust compiler normally builds source code directly into an executable.
# Internally, object code is produced, and then the (system) linker is called,
# but this all happens under the covers.
#
# However Deno's build system uses it's own linker. For it to successfully
# produce an executable from rustc-generated object code, it needs to link
# with a dozen or so "built-in" Rust libraries (as in: not Cargo crates),
# and we need to tell the linker which and where those .rlibs are.
#
# Hard-coding these libraries into the GN configuration isn't possible: the
# required .rlib files have some sort of hash code in their file name, and their
# location depends on how Rust is set up, and which toolchain is active.
#
# So instead, we have this script: it writes a list of linker options (ldflags)
# to stdout, separated by newline characters. It is called from `rust.gni` when
# GN is generating ninja files (it doesn't run in the build phase).
#
# There is no official way through which rustc will give us the information
# we need, so a "back door" is used. We tell `rustc` to compile a (dummy)
# program, and to use a custom linker. This "linker" doesn't actually link
# anything; it just dumps it's argv to a temporary file. When rustc is done,
# this script then reads the linker arguments from that temporary file, and
# then filters it to remove flags that are irrelevant or undesirable.

import json
import re
import sys
import os
from os import path
import subprocess
import tempfile


def capture_linker_args(argsfile_path):
    with open(argsfile_path, "wb") as argsfile:
        argsfile.write("\n".join(sys.argv[1:]))


def get_ldflags(rustc_args):
    # Prepare the environment for rustc.
    rustc_env = os.environ.copy()

    # We'll capture the arguments rustc passes to the linker by telling it
    # that this script *is* the linker.
    # On Posix systems, this file is directly executable thanks to it's shebang.
    # On Windows, we use a .cmd wrapper file.
    if os.name == "nt":
        rustc_linker_base, _rustc_linker_ext = path.splitext(__file__)
        rustc_linker = rustc_linker_base + ".cmd"
    else:
        rustc_linker = __file__

    # Make sure that when rustc invokes this script, it uses the same version
    # of the Python interpreter as we're currently using. On Posix systems this
    # is done making the Python directory the first element of PATH.
    # On Windows, the wrapper script uses the PYTHON_EXE environment variable.
    if os.name == "nt":
        rustc_env["PYTHON_EXE"] = sys.executable
    else:
        python_dir = path.dirname(sys.executable)
        rustc_env["PATH"] = python_dir + path.pathsep + os.environ["PATH"]

    # Create a temporary file to write captured Rust linker arguments to.
    # Unfortunately we can't use tempfile.NamedTemporaryFile here, because the
    # file it creates can't be open in two processes at the same time.
    argsfile_fd, argsfile_path = tempfile.mkstemp()
    rustc_env["ARGSFILE_PATH"] = argsfile_path

    try:
        # Build the rustc command line.
        #   * `-Clinker=` tells rustc to use our fake linker.
        #   * `-Csave-temps` prevents rustc from deleting object files after
        #     linking. We need to preserve the extra object file with allocator
        #     symbols (`_rust_alloc` etc.) in it that rustc produces.
        rustc_cmd = [
            "rustc",
            "-Clinker=" + rustc_linker,
            "-Csave-temps",
        ] + rustc_args

        # Spawn the rust compiler.
        rustc_proc = subprocess.Popen(
            rustc_cmd,
            env=rustc_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT)

        # Forward rustc's output to stderr.
        for line in rustc_proc.stdout:
            # Suppress the warning:
            #   `-C save-temps` might not produce all requested temporary
            #   products when incremental compilation is enabled.
            # It's pointless, because incremental compilation is disabled.
            if re.match(r"^warning:.*save-temps.*incremental compilation",
                        line):
                continue
            # Also, do not write completely blank lines to stderr.
            if line.strip() == "":
                continue
            sys.stderr.write(line)

        # The rustc process should return zero. If not, raise an exception.
        rustc_retcode = rustc_proc.wait()
        if rustc_retcode != 0:
            raise subprocess.CalledProcessError(rustc_retcode, rustc_cmd)

        # Read captured linker arguments from argsfile.
        argsfile_size = os.fstat(argsfile_fd).st_size
        argsfile_content = os.read(argsfile_fd, argsfile_size)
        args = argsfile_content.split("\n")

    except OSError as e:  # Note: in python 3 this will be a FileNotFoundError.
        print "Error executing rustc command (is rust installed?):"
        print " ".join(rustc_cmd) + "\n"
        raise e

    finally:
        # Close and delete the temporary file.
        os.close(argsfile_fd)
        os.unlink(argsfile_path)

    # From the list of captured linker arguments, build the list of ldflags that
    # we actually need.
    ldflags = []
    next_arg_is_flag_value = False
    for arg in args:
        # Note that within the following if/elif blocks, `pass` means that
        # that captured arguments gets included in `ldflags`. The final `else`
        # clause filters out unrecognized/unwanted flags.
        if next_arg_is_flag_value:
            # We're looking at a value that follows certain parametric flags,
            # e.g. the path in '-L <path>'.
            next_arg_is_flag_value = False
        elif arg.endswith(".rlib"):
            # Built-in Rust library, e.g. `libstd-8524caae8408aac2.rlib`.
            pass
        elif re.match(r"^empty_crate\.[a-z0-9]+\.rcgu.o$", arg):
            # This file is needed because it contains certain allocator
            # related symbols (e.g. `__rust_alloc`, `__rust_oom`).
            # The Rust compiler normally generates this file just before
            # linking an executable. We pass `-Csave-temps` to rustc so it
            # doesn't delete the file when it's done linking.
            pass
        elif arg.endswith(".crate.allocator.rcgu.o"):
            # Same as above, but for rustc version 1.29.0 and older.
            pass
        elif arg.endswith(".lib") and not arg.startswith("msvcrt"):
            # Include most Windows static/import libraries (e.g. `ws2_32.lib`).
            # However we ignore Rusts choice of C runtime (`mvcrt*.lib`).
            # Rust insists on always using the release "flavor", even in debug
            # mode, which causes conflicts with other libraries we link with.
            pass
        elif arg.upper().startswith("/LIBPATH:"):
            # `/LIBPATH:<path>`: Linker search path (Microsoft style).
            pass
        elif arg == "-l" or arg == "-L":
            # `-l <name>`: Link with library (GCC style).
            # `-L <path>`: Linker search path (GCC style).
            next_arg_is_flag_value = True  # Ensure flag argument is captured.
        elif arg == "-Wl,--start-group" or arg == "-Wl,--end-group":
            # Start or end of an archive group (GCC style).
            pass
        else:
            # Not a flag we're interested in -- don't add it to ldflags.
            continue

        ldflags += [arg]

    return ldflags


def get_version():
    version = subprocess.check_output(["rustc", "--version"])
    version = version.strip()  # Remove trailing newline.
    return version


def main():
    # If ARGSFILE_PATH is set this script is being invoked by rustc, which
    # thinks we are a linker. All we do now is write our argv to the specified
    # file and exit. Further processing is done by our grandparent process,
    # also this script but invoked by gn.
    argsfile_path = os.getenv("ARGSFILE_PATH")
    if argsfile_path is not None:
        return capture_linker_args(argsfile_path)

    empty_crate_source = path.join(path.dirname(__file__), "empty_crate.rs")

    info = {
        "version": get_version(),
        "ldflags_bin": get_ldflags([empty_crate_source]),
        "ldflags_test": get_ldflags([empty_crate_source, "--test"])
    }

    # Write the information dict as a json object.
    json.dump(info, sys.stdout)


if __name__ == '__main__':
    sys.exit(main())
