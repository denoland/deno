# rc

This contains a cross-platform reimplementation of rc.exe.

This exists mainly to compile .rc files on non-Windows hosts for cross builds.
However, it also runs on Windows for two reasons:

1. To compare the output of Microsoft's rc.exe and the reimplementation and to
    check that they produce bitwise identical output.
2. The reimplementation supports printing resource files in /showIncludes
   output, which helps getting build dependencies right.

The resource compiler consists of two parts:

1. A python script rc.py that serves as the driver.  It does unicode
   conversions, runs the input through the preprocessor, and then calls the
   actual resource compiler.
2. The resource compiler, a C++ binary obtained via sha1 files from Google
   Storage.  The binary's code currenty lives at
   https://github.com/nico/hack/tree/master/res, even though work is (slowly)
   underway to upstream it into LLVM.

To update the rc binary, run `upload_rc_binaries.sh` in this directory, on a
Mac.

rc isn't built from source as part of the regular chrome build because
it's needed in a gn toolchain tool, and these currently cannot have deps.
Alternatively, gn could be taught about deps on tools, or rc invocations could
be not a tool but a template like e.g. yasm invocations (which can have deps),
then the prebuilt binaries wouldn't be needed.
