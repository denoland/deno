This crate provides utilies to compile typescript, bundle it up, and create a V8
snapshot, all during build. This allows users to startup fast.

The cli_snapshots crate, neighboring this one uses deno_typescript at build
time.

This crate does not depend on Node, Python, nor any other external dependencies
besides those listed as such in Cargo.toml.
