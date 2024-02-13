# Deno Standard Library

[![codecov](https://codecov.io/gh/denoland/deno_std/branch/main/graph/badge.svg?token=w6s3ODtULz)](https://codecov.io/gh/denoland/deno_std)
[![ci](https://github.com/denoland/deno_std/actions/workflows/ci.yml/badge.svg)](https://github.com/denoland/deno_std/actions/workflows/ci.yml)

High-quality APIs for [Deno](https://deno.com/) and the web. Use fearlessly.

## Get Started

```ts
import { copy } from "https://deno.land/std@$STD_VERSION/fs/copy.ts";

await copy("./foo", "./bar");
```

See [here](#recommended-usage) for recommended usage patterns.

## Documentation

Check out the documentation [here](https://deno.land/std?doc).

## Recommended Usage

1. Include the version of the library in the import specifier.

   Good:
   ```ts
   import { copy } from "https://deno.land/std@$STD_VERSION/fs/copy.ts";
   ```

1. Only import modules that you require.

   Bad (when using only one function):
   ```ts
   import * as fs from "https://deno.land/std@$STD_VERSION/fs/mod.ts";
   ```

   Good (when using only one function):
   ```ts
   import { copy } from "https://deno.land/std@$STD_VERSION/fs/copy.ts";
   ```

   Good (when using multiple functions):
   ```ts
   import * as fs from "https://deno.land/std@$STD_VERSION/fs/mod.ts";
   ```

1. Do not import symbols with an underscore in the name.

   Bad:
   ```ts
   import { _format } from "https://deno.land/std@$STD_VERSION/path/_common/format.ts";
   ```

1. Do not import modules with an underscore in the path.

   Bad:
   ```ts
   import { filterInPlace } from "https://deno.land/std@$STD_VERSION/collections/_utils.ts";
   ```

1. Do not import test modules or test data.

   Bad:
   ```ts
   import { test } from "https://deno.land/std@$STD_VERSION/front_matter/test.ts";
   ```

## Stability

| Sub-module   | Status     |
| ------------ | ---------- |
| archive      | Unstable   |
| assert       | Stable     |
| async        | Stable     |
| bytes        | Stable     |
| collections  | Stable     |
| console      | Unstable   |
| csv          | Stable     |
| datetime     | Unstable   |
| dotenv       | Unstable   |
| encoding     | Unstable   |
| flags        | Unstable   |
| fmt          | Stable     |
| front_matter | Unstable   |
| fs           | Stable     |
| html         | Unstable   |
| http         | Unstable   |
| io           | Deprecated |
| json         | Stable     |
| jsonc        | Stable     |
| log          | Unstable   |
| media_types  | Stable     |
| msgpack      | Unstable   |
| path         | Unstable   |
| permissions  | Deprecated |
| regexp       | Unstable   |
| semver       | Unstable   |
| signal       | Deprecated |
| streams      | Unstable   |
| testing      | Stable     |
| toml         | Stable     |
| ulid         | Unstable   |
| url          | Unstable   |
| uuid         | Stable     |
| yaml         | Stable     |

> For background and discussions regarding the stability of the following
> sub-modules, see [#3489](https://github.com/denoland/deno_std/issues/3489).

## Deprecation Policy

We deprecate the APIs in the Standard Library when they get covered by new
JavaScript language APIs or new Web Standard APIs. These APIs are usually
removed after 3 minor versions.

If you still need to use such APIs after the removal for some reason (for
example, the usage in Fresh island), please use the URL pinned to the version
where they are still available.

For example, if you want to keep using `readableStreamFromIterable`, which was
deprecated and removed in favor of `ReadableStream.from` in `v0.195.0`, please
use the import URL pinned to `v0.194.0`:

```ts
import { readableStreamFromIterable } from "https://deno.land/std@0.194.0/streams/readable_stream_from_iterable.ts";
```

## Contributing

Check out the contributing guidelines [here](.github/CONTRIBUTING.md).

## Releases

The Standard Library is versioned independently of the Deno CLI. This will
change once the Standard Library is stabilized. See
[here](https://raw.githubusercontent.com/denoland/dotland/main/versions.json)
for the compatibility of different versions of the Deno Standard Library and the
Deno CLI.

A new minor version of the Standard Library is published at the same time as
every new version of the Deno CLI (including patch versions).
