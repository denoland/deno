# //src

Each of the directories in //src represents a major component of the Deno
project.

`//src/core` provides bindings to V8, basic op infrastructure, and exposes V8
isolate as a future. The crate is called `deno` and published to
https://crates.io/crates/deno

`//src/deno_typescript` is a crate that provides integration with the TypeScript
compiler. It's published to https://crates.io/crates/deno_typescript

`//src/js` is a crate which contains all of Deno's internal JS and TypeScript
code. During its build the TypeScript code is compiled to JavaScript. bundled
into a single file, and a V8 snapshot is created. The crate's name is
(deceptively) `deno_cli_snapshots` and published at
https://crates.io/crates/deno_cli_snapshots

`//src/cli` provides the main Deno executable. It contains a lot of integration
tests in `cli/tests`. It's published at https://crates.io/crates/deno_cli

`//src/std` provides a set of standard modules for Deno. These modules do not
get built into the Deno executable like the code in `//src/js`. Rather the
standard modules are accessable from https://deno.land/std/

Ongoing directory tree reorg:

TODO(ry) Merge deno_std repo into this repo, place it at `//src/std`

TODO(ry) Rename the `deno` crate to `deno_core`.

TODO(ry) Rename `deno_cli` to `deno` after `deno_core` rename is complete.

TODO(ry) Replace `//src/core/libdeno` with `//src/v8` a new Rust V8 binding.

TODO(ry) Remove `//tests` symlink.
