# Tools

Documentation for various tooling in support of Deno development.

## format.js

This script will format the code (currently using dprint, rustfmt). It is a
prerequisite to run this before code check in.

To run formatting:

```sh
deno run --allow-read --allow-write --allow-run ./tools/format.js
```

## lint.js

This script will lint the code base (currently using dlint, clippy). It is a
prerequisite to run this before code check in.

To run linting:

```sh
deno run --allow-read --allow-write --allow-run ./tools/lint.js
```

Tip: You can also use cargo to run the current or pending build of the deno
executable

```sh
cargo run -- run --allow-read --allow-write --allow-run ./tools/<script>
```

## wgpu_sync.js

`wgpu_sync.js` streamlines updating `deno_webgpu` from
[gfx-rs/wgpu](https://github.com/gfx-rs/wgpu/).

It essentially vendors the `deno_webgpu` tree with a few minor patches applied
on top, somewhat similar to `git subtree`.

1. Update `COMMIT` or `V_WGPU` in `./tools/wgpu_sync.js`
2. Run `./tools/wgpu_sync.js`
3. Double check changes, possibly patch
4. Commit & send a PR with the updates

## copyright_checker.js

`copyright_checker.js` is used to check copyright headers in the codebase.

To run the _copyright checker_:

```sh
deno run --allow-read --allow-run  ./tools/copyright_checker.js
```

Then it will check all code files in the repository and report any files that
are not properly licensed.
