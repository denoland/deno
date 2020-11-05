# Tools

Documentation for various tooling in support of Deno development.

## format.js

This script will format the code (currently using dprint, rustfmt). It is a
prerequisite to run this before code check in.

To run formatting:

```sh
deno run -A --unstable ./tools/format.js
```

## lint.py

This script will lint the code base (currently using dlint, clippy). It is a
prerequisite to run this before code check in.

To run linting:

```sh
deno run -A --unstable ./tools/lint.js
```
