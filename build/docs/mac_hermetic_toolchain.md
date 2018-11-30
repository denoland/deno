# Mac and iOS hermetic toolchain instructions

The following is a short explanation of why we use a the hermetic toolchain
and instructions on how to roll a new toolchain.

## How to roll a new hermetic toolchain.

1. Download a new version of Xcode, and confirm either mac or ios builds
   properly with this new version.

2. Run the following command:

   ```
   src/build/package_mac_toolchain.py /path/to/Xcode.app/ [ios|mac]
   ```

   The script will create a subset of the toolchain necessary for a build, and
   upload them to be used by hermetic builds.

   If for some reason this toolchain version has already been uploaded, the
   script will ask if we should create sub revision.  This can be necessary when
   the package script has been updated to compress additional files.

2. Create a CL with updated [MAC|IOS]_TOOLCHAIN_VERSION and _SUB_REVISION in
   src/build/mac_toolchain.py with the version created by the previous command.

3. Run the CL through the trybots to confirm the roll works.

## Why we use a hermetic toolchain.

Building Chrome Mac currently requires many binaries that come bundled with
Xcode, as well the macOS and iphoneOS SDK [also bundled with Xcode].  Note that
Chrome ships its own version of clang [compiler], but is dependent on Xcode
for these other binaries.

Chrome should be built against the latest SDK available, but historically,
updating the SDK has been nontrivially difficult.  Additionally, bot system
installs can range from Xcode 5 on some bots, to the latest and
greatest.  Using a hermetic toolchain has two main benefits:

1. Build Chrome with a well-defined toolchain [rather than whatever happens to
be installed on the machine].

2. Easily roll/update the toolchain.
