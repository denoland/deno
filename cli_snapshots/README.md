This is a small crate which exports just a few static blobs. It contains a
build.rs file which compiles Deno's internal JavaScript and TypeScript code
first into a single AMD bundle, and then into a binary V8 Snapshot.

The main Deno executable crate ("cli") depends on this crate and has access to
all the runtime code.

The //js/ directory should be moved as a sub-directory of this crate, to denote
the dependency structure. However, that is left to future work.
