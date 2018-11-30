# Incremental Install

Incremental Install is a way of building & deploying an APK that tries to
minimize the time it takes to make a change and see that change running on
device. They work best with `is_component_build=true`, and do *not* require a
rooted device.

## Building

**Option 1:** Add the gn arg:

    incremental_apk_by_default = true

This causes all apks to be built as incremental (except for blacklisted ones).

**Option 2:** Add `_incremental` to the apk target name. E.g.:

    ninja -C out/Debug chrome_public_apk_incremental
    ninja -C out/Debug chrome_public_test_apk_incremental

## Running

It is not enough to `adb install` them. You must use a generated wrapper script:

    out/Debug/bin/install_chrome_public_apk_incremental
    out/Debug/bin/run_chrome_public_test_apk_incremental  # Automatically sets --fast-local-dev

## Caveats

Isolated processes (on L+) are incompatible with incremental install. As a
work-around, you can disable isolated processes only for incremental apks using
gn arg:

    disable_incremental_isolated_processes = true

# How it Works

## Overview

The basic idea is to side-load .dex and .so files to `/data/local/tmp` rather
than bundling them in the .apk. Then, when making a change, only the changed
.dex / .so needs to be pushed to the device.

Faster Builds:

 * No `final_dex` step (where all .dex files are merged into one)
 * No need to rebuild .apk for code-only changes (but required for resources)
 * Apks sign faster because they are smaller.

Faster Installs:

 * The .apk is smaller, and so faster to verify.
 * No need to run `adb install` for code-only changes.
 * Only changed .so / .dex files are pushed. MD5s of existing on-device files
   are cached on host computer.

Slower Initial Runs:

 * The first time you run an incremental .apk, the `DexOpt` needs to run on all
   .dex files. This step is normally done during `adb install`, but is done on
   start-up for incremental apks.
   * DexOpt results are cached, so subsequent runs are much faster

## The Code

All incremental apks have the same classes.dex, which is built from:

    //build/android/incremental_install:bootstrap_java

They also have a transformed `AndroidManifest.xml`, which overrides the the
main application class and any instrumentation classes so that they instead
point to `BootstrapApplication`. This is built by:

    //build/android/incremental_install/generate_android_manifest.py

Wrapper scripts and install logic is contained in:

    //build/android/incremental_install/create_install_script.py
    //build/android/incremental_install/installer.py

Finally, GN logic for incremental apks is sprinkled throughout.
