# Overview

This directory contains files that make it possible for Linux
distributions to build Chromium using system toolchain.

For more info on established way such builds are configured,
please read the following:

 - https://www.gnu.org/software/make/manual/html_node/Implicit-Variables.html

Why do distros want CFLAGS, LDFLAGS, etc? Please read the following
for some examples. This is not an exhaustive list.

 - https://wiki.debian.org/Hardening
 - https://wiki.ubuntu.com/DistCompilerFlags
 - https://fedoraproject.org/wiki/Changes/Harden_All_Packages
 - https://fedoraproject.org/wiki/Changes/Modernise_GCC_Flags
 - https://fedoraproject.org/wiki/Packaging:Guidelines#Compiler_flags
 - https://blog.flameeyes.eu/2010/09/are-we-done-with-ldflags/
 - https://blog.flameeyes.eu/2008/08/flags-and-flags/

# Usage

Add the following to GN args:

```
custom_toolchain="//build/toolchain/linux/unbundle:default"
host_toolchain="//build/toolchain/linux/unbundle:default"
```

See [more docs on GN](https://gn.googlesource.com/gn/+/master/docs/quick_start.md).

To cross-compile (not fully tested), add the following:

```
host_toolchain="//build/toolchain/linux/unbundle:host"
v8_snapshot_toolchain="//build/toolchain/linux/unbundle:host"
```

Note: when cross-compiling for a 32-bit target, a matching 32-bit toolchain
may be needed.
