#!/usr/bin/env bash
# Build Deno on macOS (aarch64) with lzld — a thin ld64.lld wrapper that strips
# `-framework`/`-weak_framework` and links liblzld instead, so system frameworks
# (CoreFoundation/Foundation/Security/CoreServices/Metal/QuartzCore/...) are
# dlopen'd on first use rather than loaded at process launch. Cuts ~0.5-1ms of
# dyld startup cost. See tools/lzld (git submodule) for the mechanism.
#
# This is opt-in: `cargo build` is unchanged. `-fuse-ld` requires an *absolute*
# linker path (Apple clang rejects relative ones), which can't be committed
# portably — so we compute it here at invocation time and inject it via
# `cargo --config`, leaving .cargo/config.toml untouched.
#
# Usage:  tools/build_lzld.sh [extra cargo args]   (default: --release --bin deno)
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
  echo "lzld build is aarch64-macOS only" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
lzld_dir="$root/tools/lzld"

if [ ! -f "$lzld_dir/lzld" ]; then
  echo "tools/lzld is empty — run: git submodule update --init tools/lzld" >&2
  exit 1
fi

# (Re)build the lazy-load shim for this arch.
make -C "$lzld_dir" >/dev/null

# Mirror the committed aarch64 rustflags (--icf=safe), swapping the default
# linker for the lzld wrapper. The committed [target.'cfg(all())'] flags
# (clippy lints, --cfg) are a separate key and remain in effect.
link_args="-fuse-ld=$lzld_dir/lzld -Wl,--icf=safe -L$lzld_dir -llzld_$(uname -m)"

args=("$@")
if [ ${#args[@]} -eq 0 ]; then
  args=(--release --bin deno)
fi

exec cargo build \
  --config "target.aarch64-apple-darwin.rustflags=[\"-C\", \"link-args=$link_args\"]" \
  "${args[@]}"
