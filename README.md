# About
`//build` contains:
 * Core GN templates and configuration
 * Core Python build scripts

Since this directory is DEPS'ed in by some other repositories (webrtc, pdfium,
v8, etc), it should be kept as self-contained as possible by not referring
to files outside of it. Some exceptions exist (`//testing`, select
`//third_party` subdirectories), but new dependencies tend to break these other
projects, and so should be avoided.

## Contents
 * `//build/config` - Common templates via `.gni` files.
 * `//build/toolchain` - GN toolchain definitions.
 * `Other .py files` - Some are used by GN/Ninja. Some by gclient hooks, some
   are just random utilities.

Files referenced by `//.gn`:
 * `//build/BUILDCONFIG.gn` - Included by all `BUILD.gn` files.
 * `//build/secondary` - An overlay for `BUILD.gn` files. Enables adding
   `BUILD.gn` to directories that live in sub-repositories.
 * `//build_overrides` -
   Refer to [//build_overrides/README.md](../build_overrides/README.md).

## Docs

* [Writing GN Templates](docs/writing_gn_templates.md)
* [Debugging Slow Builds](docs/debugging_slow_builds.md)
* [Mac Hermetic Toolchains](docs/mac_hermetic_toolchain.md)
* [Android Build Documentation](android/docs)
