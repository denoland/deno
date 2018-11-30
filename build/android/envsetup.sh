#!/bin/bash
# Copyright (c) 2012 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# Adds Android SDK tools and related helpers to PATH, useful for development.
# Not used on bots, nor required for any commands to succeed.
# Use like: source build/android/envsetup.sh

# Make sure we're being sourced.
if [[ -n "$BASH_VERSION" && "${BASH_SOURCE:-$0}" == "$0" ]]; then
  echo "ERROR: envsetup must be sourced."
  exit 1
fi

# This only exists to set local variables. Don't call this manually.
android_envsetup_main() {
  local SCRIPT_PATH="$1"
  local SCRIPT_DIR="$(dirname "$SCRIPT_PATH")"
  local CHROME_SRC="$(readlink -f "${SCRIPT_DIR}/../../")"
  local ANDROID_SDK_ROOT="${CHROME_SRC}/third_party/android_tools/sdk/"

  export PATH=$PATH:${ANDROID_SDK_ROOT}/platform-tools
  export PATH=$PATH:${ANDROID_SDK_ROOT}/tools/
  export PATH=$PATH:${CHROME_SRC}/build/android
}
# In zsh, $0 is the name of the file being sourced.
android_envsetup_main "${BASH_SOURCE:-$0}"
unset -f android_envsetup_main
