# Standard library

Deno provides a set of standard modules that are audited by the core team and
are guaranteed to work with Deno.

Standard library is available at: https://deno.land/std/

## Versioning and stability

Standard library is not yet stable and therefore it is versioned differently
than Deno. For latest release consult https://deno.land/std/ or
https://deno.land/std/version.ts. The standard library is released each time
Deno is released.

We strongly suggest to always use imports with pinned version of standard
library to avoid unintended changes. For example, rather than linking to the
default branch of code, which may change at any time, potentially causing
compilation errors or unexpected behavior:

```typescript
// import the latest release, this should be avoided
import { copy } from "https://deno.land/std/fs/copy.ts";
```

instead, used a version of the std library which is immutable and will not
change:

```typescript
// imports from v$STD_VERSION of std, never changes
import { copy } from "https://deno.land/std@$STD_VERSION/fs/copy.ts";
```

## Troubleshooting

Some of the modules provided in standard library use unstable Deno APIs.

Trying to run such modules without `--unstable` CLI flag ends up with a lot of
TypeScript errors suggesting that some APIs in the `Deno` namespace do not
exist:

```typescript
// main.ts
import { copy } from "https://deno.land/std@$STD_VERSION/fs/copy.ts";

copy("log.txt", "log-old.txt");
```

```shell
$ deno run --allow-read --allow-write main.ts
Compile file:///dev/deno/main.ts
Download https://deno.land/std@$STD_VERSION/fs/copy.ts
Download https://deno.land/std@$STD_VERSION/fs/ensure_dir.ts
Download https://deno.land/std@$STD_VERSION/fs/_util.ts
error: TS2339 [ERROR]: Property 'utime' does not exist on type 'typeof Deno'. 'Deno.utime' is an unstable API. Did you forget to run with the '--unstable' flag?
    await Deno.utime(dest, statInfo.atime, statInfo.mtime);
               ~~~~~
    at https://deno.land/std@$STD_VERSION/fs/copy.ts:92:16

TS2339 [ERROR]: Property 'utimeSync' does not exist on type 'typeof Deno'. 'Deno.utimeSync' is an unstable API. Did you forget to run with the '--unstable' flag?
    Deno.utimeSync(dest, statInfo.atime, statInfo.mtime);
         ~~~~~~~~~
    at https://deno.land/std@$STD_VERSION/fs/copy.ts:103:10
```

Solution to that problem requires adding `--unstable` flag:

```shell
deno run --allow-read --allow-write --unstable main.ts
```

To make sure that API producing error is unstable check
[`lib.deno.unstable.d.ts`](https://github.com/denoland/deno/blob/$CLI_VERSION/cli/dts/lib.deno.unstable.d.ts)
declaration.

This problem should be fixed in the near future. Feel free to omit the flag if
the particular modules you depend on compile successfully without it.
