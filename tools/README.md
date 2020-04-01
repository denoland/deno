# Tools

Documentation for various tooling in support of Deno development

## docs.py

This script is used to generate the API documentation for Deno. It can be useful
to run locally to test the formatting of your changes to the documentation.

If you would like to see how your JSDoc will be rendered after changing
`cli/js/lib.deno.ns.d.ts`, you can run the following:

First, make sure you have typedoc installed:

```bash
npm install typedoc --save-dev
```

Then run the doc generation tool:

```bash
./tools/docs.py
```

Output can be found in `./target/typedoc/index.html`

## format.py

This script will format the code (currently using prettier, yapf and rustfmt).
It is a prerequisite to run this before code check in.

To run formatting:

```bash
./tools/format.py
```

## lint.py

This script will lint the code base (currently using eslint, pylint and clippy).
It is a prerequisite to run this before code check in.

To run linting:

```bash
./tools/lint.py
```
