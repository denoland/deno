This crate is a work in progress:

1. Add a clippy.toml file that bans accessing the file system directory and
   instead does it through a trait.
1. Make this crate work in Wasm.
1. Refactor to store npm packument in a single place:
   https://github.com/denoland/deno/issues/27198
