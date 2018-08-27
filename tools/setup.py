#!/usr/bin/env python
import third_party
from util import run, root_path, build_path, build_mode
import os
import sys
from distutils.spawn import find_executable


def main():
    os.chdir(root_path)

    third_party.fix_symlinks()
    third_party.download_gn()
    third_party.download_clang_format()
    third_party.download_clang()
    third_party.maybe_download_sysroot()

    write_lastchange()

    mode = build_mode(default=None)
    if mode is not None:
        gn_gen(mode)
    else:
        gn_gen("release")
        gn_gen("debug")


def write_lastchange():
    run([
        sys.executable, "build/util/lastchange.py", "-o",
        "build/util/LASTCHANGE", "--source-dir", root_path,
        "--filter="
    ])


def get_gn_args():
    out = []
    if build_mode() == "release":
        out += ["is_official_build=true"]
    elif build_mode() == "debug":
        pass
    else:
        print "Bad mode {}. Use 'release' or 'debug' (default)" % build_mode()
        sys.exit(1)
    if "DENO_BUILD_ARGS" in os.environ:
        out += os.environ["DENO_BUILD_ARGS"].split()

    # Check if ccache or sccache are in the path, and if so we set cc_wrapper.
    cc_wrapper = find_executable("ccache") or find_executable("sccache")
    if cc_wrapper:
        out += [r'cc_wrapper="%s"' % cc_wrapper]
        # Windows needs a custom toolchain for cc_wrapper to work.
        if os.name == "nt":
            out += [
                'custom_toolchain="//build_extra/toolchain/win:win_clang_x64"'
            ]

    print "DENO_BUILD_ARGS:", out

    return out


# gn gen.
def gn_gen(mode):
    os.environ["DENO_BUILD_MODE"] = mode

    gn_args = get_gn_args()

    # mkdir $build_path(). We do this so we can write args.gn before running gn gen.
    if not os.path.isdir(build_path()):
        os.makedirs(build_path())

    # Rather than using gn gen --args we manually write the args.gn override file.
    # This is to avoid quoting/escaping complications when passing overrides as
    # command-line arguments.
    args_filename = os.path.join(build_path(), "args.gn")
    if not os.path.exists(args_filename) or gn_args:
        with open(args_filename, "w+") as f:
            f.write("\n".join(gn_args) + "\n")

    run([third_party.gn_path, "gen", build_path()],
        env=third_party.google_env())


if __name__ == '__main__':
    sys.exit(main())
