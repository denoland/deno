#!/bin/bash
# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
set -eu

# Builds new rc binaries at head and uploads them to google storage.
# The new .sha1 files will be in the tree after this has run.

if [[ "$OSTYPE" != "darwin"* ]]; then
  echo "this script must run on a mac"
  exit 1
fi

DIR="$(cd "$(dirname "${0}" )" && pwd)"
SRC_DIR="$DIR/../../../.."

# Make sure Linux and Windows sysroots are installed, for distrib.py.
$SRC_DIR/build/linux/sysroot_scripts/install-sysroot.py --arch amd64
$SRC_DIR/build/vs_toolchain.py update --force

# Make a temporary directory.
WORK_DIR=$(mktemp -d)
if [[ ! "$WORK_DIR" || ! -d "$WORK_DIR" ]]; then
  echo "could not create temp dir"
  exit 1
fi
function cleanup {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

# Check out rc and build it in the temporary directory. Copy binaries over.
pushd "$WORK_DIR" > /dev/null
git clone -q https://github.com/nico/hack
cd hack/res
./distrib.py "$SRC_DIR"
popd > /dev/null
cp "$WORK_DIR/hack/res/rc-linux64" "$DIR/linux64/rc"
cp "$WORK_DIR/hack/res/rc-mac" "$DIR/mac/rc"
cp "$WORK_DIR/hack/res/rc-win.exe" "$DIR/win/rc.exe"

# Upload binaries to cloud storage.
upload_to_google_storage.py -b chromium-browser-clang/rc "$DIR/linux64/rc"
upload_to_google_storage.py -b chromium-browser-clang/rc "$DIR/mac/rc"
upload_to_google_storage.py -b chromium-browser-clang/rc "$DIR/win/rc.exe"
