# Deno Standard Modules

[![Build Status](https://dev.azure.com/denoland/deno_std/_apis/build/status/denoland.deno_std?branchName=master)](https://dev.azure.com/denoland/deno_std/_build/latest?definitionId=2?branchName=master)

These modules do not have external dependencies and they are reviewed by the
Deno core team. The intention is to have a standard set of high quality code
that all Deno projects can use fearlessly.

Contributions are welcome!

## How to use

These modules are tagged in accordance with Deno releases. So, for example, the
v0.3.0 tag is guaranteed to work with deno v0.3.0.
You can link to v0.3.0 using the URL `https://deno.land/std@v0.3.0/`

It's strongly recommended that you link to tagged releases rather than the
master branch. The project is still young and we expect disruptive renames in
the future.

## Documentation

Here are the dedicated documentations of modules:

- [colors](colors/README.md)
- [datetime](datetime/README.md)
- [encoding](encoding/README.md)
- [examples](examples/README.md)
- [flags](flags/README.md)
- [fs](fs/README.md)
- [http](http/README.md)
- [log](log/README.md)
- [media_types](media_types/README.md)
- [prettier](prettier/README.md)
- [strings](strings/README.md)
- [testing](testing/README.md)
- [ws](ws/README.md)

## Contributing

deno_std is a loose port of [Go's standard library](https://golang.org/pkg/).
When in doubt, simply port Go's source code, documentation, and tests. There
are many times when the nature of JavaScript, TypeScript, or Deno itself
justifies diverging from Go, but if possible we want to leverage the energy that
went into building Go. We generally welcome direct ports of Go's code.

Please ensure the copyright headers cite the code's origin.

Follow the [style guide](https://deno.land/style_guide.html).
