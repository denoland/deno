# Tools

Documentation for various tooling in support of Deno development

## format.py

This script will format the code (currently using dprint, yapf and rustfmt). It
is a prerequisite to run this before code check in.

To run formatting:

```sh
./tools/format.py
```

## lint.py

This script will lint the code base (currently using eslint, pylint and clippy).
It is a prerequisite to run this before code check in.

To run linting:

```sh
./tools/lint.py
```
