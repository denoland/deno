# Crate: `deno_cli_snapshots`

## AKA `cli_snapshots` AKA `//js`

This is a small crate which exports just a few static blobs. It contains a
build.rs file which compiles Deno's internal JavaScript and TypeScript code
first into a single AMD bundle, and then into a binary V8 Snapshot.

The main Deno executable crate ("cli") depends on this crate and has access to
all the runtime code.
