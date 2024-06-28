# Faster rebuilds with [`cargo-plonk`](https://crates.io/crates/cargo-plonk)

This document describes how to use `cargo plonk` to speed up Deno rebuilds for
faster dev cycles.

```
cargo install cargo-plonk
```

## How it works

Plonk works by hot swapping symbols using a fresh dynamic library of the local
crates.

## Usage

First, compile Deno normally.

```
cargo build -p deno [--release]
```

Run the following command to start watching for changes in `ext/webgpu` crate
and hot swap `init_ops_and_esm` function into the previously built `deno` bin.

```
cargo plonk run \
  --package deno_webgpu \
  --symbol init_ops_and_esm \
  --bin deno \
  --watch
```

> Important:
>
> Currently, this will only works for symbols that have been "materialized" in
> their crates. Cross-crate generics will not work.

You can use `cargo plonk run` to re-run commands on changes.

```
cargo plonk run -v \
  -p deno_webgpu \
  -s init_ops_and_esm \
  -b deno \
  --watch \
  -- eval "await navigator.gpu.requestAdapter()" --unstable
```

Comparing incremental compile times for `ext/webgpu` on Mac M1:

| profile   | `cargo build` | `cargo plonk build` |
| --------- | ------------- | ------------------- |
| `debug`   | 42 s          | 0.5s                |
| `release` | 5 mins 12 s   | 2s                  |

## Debugging

Use the `-v`/`--verbose` flag to turn on debug info.

```
    Finished dev [unoptimized + debuginfo] target(s) in 8.86s
[*] Running: DYLD_INSERT_LIBRARIES="/Users/divy/gh/plonk/target/release/build/cargo-plonk-dd0f08c90ca82109/out/inject.dylib" DYLD_LIBRARY_PATH="/Users/divy/.rustup/toolchains/1.75.0-aarch64-apple-darwin/lib" NEW_SYMBOL="_ZN11deno_webgpu11deno_webgpu16init_ops_and_esm17h683ed96f45027bc1E" PLONK_BINARY="/Users/divy/gh/deno/target/debug/deno" PLONK_LIBRARY="/Users/divy/gh/deno/target/debug/libdeno_webgpu.dylib" SYMBOL="_ZN11deno_webgpu11deno_webgpu16init_ops_and_esm17h6907fcd8be7e215eE" VERBOSE="y" "/Users/divy/gh/deno/target/debug/deno" "eval" "await navigator.gpu.requestAdapter()" "--unstable"
[*] Plonking _ZN11deno_webgpu11deno_webgpu16init_ops_and_esm17h6907fcd8be7e215eE in /Users/divy/gh/deno/target/debug/libdeno_webgpu.dylib
[*] Old address: 0x105fcff2c
[*] New address: 0x128511424
===
```

Report any bugs and feature requests in the `cargo-plonk` issue tracker:
https://github.com/littledivy/plonk/issues/new
