# Deno

A secure runtime for JavaScript and TypeScript built with V8, Rust, and Tokio.

[![Build Status](https://github.com/denoland/deno/workflows/build/badge.svg)](https://github.com/denoland/deno/actions)

Deno aims to provide a productive and secure scripting environment for the
modern programmer. It is built on top of V8, Rust, and TypeScript.

Please read the [introduction](https://deno.land/manual.html#introduction) for
more specifics.

[Website](https://deno.land/)

[Manual](https://deno.land/manual.html)

[Install](https://github.com/denoland/deno_install)

[API Reference](https://deno.land/typedoc/)

[Style Guide](https://deno.land/style_guide.html)

[Module Repository](https://deno.land/x/)

[Releases](Releases.md)

[Chat](https://gitter.im/denolife/Lobby)

[More links](https://github.com/denolib/awesome-deno)

## Code Organization

Each of the top-level directories represents a major component of the Deno
project.

`//core` provides bindings to V8, basic op infrastructure, and exposes V8
isolate as a future. The crate is called `deno_core` and published to
https://crates.io/crates/deno_core

`//ts` is a crate that provides integration with the TypeScript compiler. It's
published to https://crates.io/crates/deno_typescript

`//cli` provides the main Deno executable. It contains a lot of integration
tests in `//cli/tests`. It's published at https://crates.io/crates/deno_cli

`//std` provides a set of standard modules for Deno. These modules do not get
built into the Deno executable like the code in `//rt`. Rather the standard
modules are accessable from https://deno.land/std/

Ongoing directory tree reorg:

TODO(ry) Merge deno_std repo into this repo, place it at `//std`

TODO(ry) Rename `deno_cli` to `deno` after `deno_core` rename is complete.

TODO(ry) Replace `//core/libdeno` with `//v8` a new Rust V8 binding.

TODO(ry) Remove `//tests` symlink.
