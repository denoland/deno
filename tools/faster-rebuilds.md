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
| `debug`   | 1 mins 10 s   | 0.5s                |
| `release` | 5 mins 12 s   | 2s                  |
