# Deno Standard Modules

These modules do not have external dependencies and they are reviewed by the
Deno core team. The intention is to have a standard set of high quality code
that all Deno projects can use fearlessly.

Contributions are welcome!

## How to use

These modules are tagged in accordance with Deno releases. So, for example, the
v0.3.0 tag is guaranteed to work with deno v0.3.0. You can link to v0.3.0 using
the URL `https://deno.land/std@v0.3.0/`. Not specifying a tag will link to the
master branch.

It is strongly recommended that you link to tagged releases to avoid unintended
updates.

Don't link to / import any module whose path:

- Has a name or parent with an underscore prefix: `_foo.ts`, `_util/bar.ts`.
- Is that of a test module or test data: `test.ts`, `foo_test.ts`,
  `testdata/bar.txt`.

No stability is guaranteed for these files.

## Documentation

To browse documentation for modules:

- Go to https://deno.land/std/.
- Navigate to any module of interest.
- Click the "DOCUMENTATION" link.

## Contributing

deno_std is a loose port of [Go's standard library](https://golang.org/pkg/).
When in doubt, simply port Go's source code, documentation, and tests. There are
many times when the nature of JavaScript, TypeScript, or Deno itself justifies
diverging from Go, but if possible we want to leverage the energy that went into
building Go. We generally welcome direct ports of Go's code.

Please ensure the copyright headers cite the code's origin.

Follow the
[style guide](https://github.com/denoland/deno/blob/master/docs/contributing/style_guide.md).
