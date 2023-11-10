# Tools

Documentation for various tooling in support of Deno development.

## format.js

This script will format the code (currently using dprint, rustfmt). It is a
prerequisite to run this before code check in.

To run formatting:

```sh
deno run --allow-read --allow-write --allow-run --unstable ./tools/format.js
```

## lint.js

This script will lint the code base (currently using dlint, clippy). It is a
prerequisite to run this before code check in.

To run linting:

```sh
deno run --allow-read --allow-write --allow-run --unstable ./tools/lint.js
```

Tip: You can also use cargo to run the current or pending build of the deno
executable

```sh
cargo run -- run --allow-read --allow-write --allow-run --unstable ./tools/<script>
```

## copyright_checker.js

`copyright_checker.js` is used to check copyright headers in the codebase.

To run the _copyright checker_:

```sh
deno run --allow-read --allow-run --unstable  ./tools/copyright_checker.js
```

Then it will check all code files in the repository and report any files that
are not properly licensed.
