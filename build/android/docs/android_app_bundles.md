# Introduction

This document describes how the Chromium build system supports Android app
bundles.

[TOC]

# Overview of app bundles

An Android app bundle is an alternative application distribution format for
Android applications on the Google Play Store, that allows reducing the size
of binaries sent for installation to individual devices that run on Android L
and beyond. For more information about them, see the official Android
documentation at:

  https://developer.android.com/guide/app-bundle/

For the context of this document, the most important points are:

  - Unlike a regular APK (e.g. `foo.apk`), the bundle (e.g. `foo.aab`) cannot
    be installed directly on a device.

  - Instead, it must be processed into a set of installable split APKs, which
    are stored inside a special zip archive (e.g. `foo.apks`).

  - The splitting can be based on various criteria: e.g. language or screen
    density for resources, or cpu ABI for native code.

  - The bundle also uses the notion of modules to separate several application
    features. Each module has its own code, assets and resources, and can be
    installed separately from the rest of the application if needed.

  - The main application itself is stored in the '`base`' module (this name
    cannot be changed).


# Declaring app bundles with GN templates

Here's an example that shows how to declare a simple bundle that contains
a single base module, which enables language-based splits:

```gn

  # First declare the first bundle module. The base module is the one
  # that contains the main application's code, resources and assets.
  android_app_bundle_module("foo_base_module") {
    # Declaration are similar to android_apk here.
    ...
  }

  # Second, declare the bundle itself.
  android_app_bundle("foo_bundle") {
    # Indicate the base module to use for this bundle
    base_module_target = ":foo_base_module"

    # The name of our bundle file (without any suffix). Default would
    # be 'foo_bundle' otherwise.
    bundle_name = "FooBundle"

    # Signing your bundle is required to upload it to the Play Store
    # but since signing is very slow, avoid doing it for non official
    # builds. Signing the bundle is not required for local testing.
    sign_bundle = is_official_build

    # Enable language-based splits for this bundle. Which means that
    # resources and assets specific to a given language will be placed
    # into their own split APK in the final .apks archive.
    enable_language_splits = true

    # Proguard settings must be passed at the bundle, not module, target.
    proguard_enabled = !is_java_debug
  }
```

When generating the `foo_bundle` target with Ninja, you will end up with
the following:

  - The bundle file under `out/Release/apks/FooBundle.aab`

  - A helper script called `out/Release/bin/foo_bundle`, which can be used
    to install / launch / uninstall the bundle on local devices.

    This works like an APK wrapper script (e.g. `foo_apk`). Use `--help`
    to see all possible commands supported by the script.

If you need more modules besides the base one, you will need to list all the
extra ones using the extra_modules variable which takes a list of GN scopes,
as in:

```gn

  android_app_bundle_module("foo_base_module") {
    ...
  }

  android_app_bundle_module("foo_extra_module") {
    ...
  }

  android_app_bundle("foo_bundle") {
    base_module_target = ":foo_base_module"

    extra_modules = [
      { # NOTE: Scopes require one field per line, and no comma separators.
        name = "my_module"
        module_target = ":foo_extra_module"
      }
    ]

    ...
  }
```

Note that each extra module is identified by a unique name, which cannot
be '`base`'.


# Bundle signature issues

Signing an app bundle is not necessary, unless you want to upload it to the
Play Store. Since this process is very slow (it uses `jarsigner` instead of
the much faster `apkbuilder`), you can control it with the `sign_bundle`
variable, as described in the example above.

The `.apks` archive however always contains signed split APKs. The keystore
path/password/alias being used are the default ones, unless you use custom
values when declaring the bundle itself, as in:

```gn
  android_app_bundle("foo_bundle") {
    ...
    keystore_path = "//path/to/keystore"
    keystore_password = "K3y$t0Re-Pa$$w0rd"
    keystore_name = "my-signing-key-name"
  }
```

These values are not stored in the bundle itself, but in the wrapper script,
which will use them to generate the `.apks` archive for you. This allows you
to properly install updates on top of existing applications on any device.


# Proguard and bundles

When using an app bundle that is made of several modules, it is crucial to
ensure that proguard, if enabled:

- Keeps the obfuscated class names used by each module consistent.
- Does not remove classes that are not used in one module, but referenced
  by others.

To achieve this, a special scheme called *synchronized proguarding* is
performed, which consists of the following steps:

- The list of unoptimized .jar files from all modules are sent to a single
  proguard command. This generates a new temporary optimized *group* .jar file.

- Each module extracts the optimized class files from the optimized *group*
  .jar file, to generate its own, module-specific, optimized .jar.

- Each module-specific optimized .jar is then sent to dex generation.

This synchronized proguarding step is added by the `android_app_bundle()` GN
template. In practice this means the following:

  - If `proguard_enabled` and `proguard_jar_path` must be passed to
    `android_app_bundle` targets, but not to `android_app_bundle_module` ones.

  - `proguard_configs` can be still passed to individual modules, just
    like regular APKs. All proguard configs will be merged during the
    synchronized proguard step.


# Manual generation and installation of .apks archives

Note that the `foo_bundle` script knows how to generate the .apks archive
from the bundle file, and install it to local devices for you. For example,
to install and launch a bundle, use:

```sh
  out/Release/bin/foo_bundle run
```

If you want to manually look or use the `.apks` archive, use the following
command to generate it:

```sh
  out/Release/bin/foo_bundle build-bundle-apks \
      --output-apks=/tmp/BundleFoo.apks
```

All split APKs within the archive will be properly signed. And you will be
able to look at its content (with `unzip -l`), or install it manually with:

```sh
  build/android/gyp/bundletool.py install-apks \
      --apks=/tmp/BundleFoo.apks \
      --adb=$(which adb)
```
