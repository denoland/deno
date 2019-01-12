# Deno Standard Modules

[![Build Status](https://dev.azure.com/denoland/deno_std/_apis/build/status/denoland.deno_std?branchName=master)](https://dev.azure.com/denoland/deno_std/_build/latest?definitionId=2?branchName=master)

- **[colors](./colors/)**

  Modules that generate ANSI color codes for the console.

- **[flags](./flags/)**

  Command line arguments parser.

- **[logging](./logging/)**

  Command line logging

- **[media_types](./media_types/)**

  A library for resolving media types (MIME types) and extensions.

- **[mkdirp](./fs/)**

  Make directory branches.

- **[net](./net/)**

  A framework for creating HTTP/HTTPS servers inspired by GoLang.

- **[path](./fs/path)**

  File path manipulation.

- **[testing](./testing/)**

  Testing

## Style Guide

### Use the term "module" instead of "library" or "package"

For clarity and consistency avoid the terms "library" and "package". Instead use
"module" to refer to a single JS or TS file and also to refer to a directory of
TS/JS code.

### Use the filename "mod.ts" as the default entry point to a directory of code

`index.ts` comes with the wrong connotations - and `main.ts` should be reserved
for executable programs. The filename `mod.ts` follows Rust’s convention, is
shorter than `index.ts`, and doesn’t come with any preconceived notions about
how it might work.

### Within `deno_std`, do not depend on external code

`deno_std` is intended to be baseline functionality that all Deno programs can
rely on. We want to guarantee to users that this code does not include
potentially unreviewed third party code.

### Within `deno_std`, minimize dependencies; do not make circular imports.

Although `deno_std` is a standalone codebase, we must still be careful to keep
the internal dependencies simple and manageable. In particular, be careful to
not to introduce circular imports.

### For consistency, use underscores, not dashes in filenames.

Example: Instead of `file-server.ts` use `file_server.ts`.

### Format code according using prettier.

More specifically, code should be wrapped at 80 columns and use 2-space
indentation and use camel-case. Use `//format.ts` to invoke prettier.

### Use JS Doc to document exported machinery

We strive for complete documentation. Every exported symbol ideally should have
a documentation line.

If possible, use a single line for the JS Doc. Example:

```ts
/** foo does bar. */
export function foo() {
  // ...
}
```

See [CONTRIBUTING.md](https://github.com/denoland/deno/blob/master/.github/CONTRIBUTING.md#documenting-apis)
for more details.

### TODO Comments

TODO comments should be include an issue or the author's github username in
parentheses. Example:

```
// TODO(ry) Add tests.
// TODO(#123) Support Windows.
```

### Copyright headers

Most files in `deno_std` should have the following copyright header:

```
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
```

If the code originates elsewhere, ensure that the file has the proper copyright
headers. We only allow MIT, BSD, and Apache licensed code in `deno_std`.

### Top level functions should not use arrow syntax

Top level functions should use the `function` keyword. Arrow syntax should be
limited to closures.

Bad

```
export const foo(): string => {
  return "bar";
}
```

Good

```
export function foo(): string {
  return "bar";
}
```

### When referencing Deno online, use the #denoland tag.

The name "deno" unfortunately is not especially unique on the internet. In order
to centralize the community, please tag github project, tweet, and other content
with `#denoland`.

---

Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
