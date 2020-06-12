# Standard library

Deno provides a set of standard modules that are audited by the core team and
are guaranteed to work with Deno.

Standard library is available at: https://deno.land/std/

## Versioning and stability

Standard library is not yet stable and therefore it is versioned differently
than Deno. For latest release consult https://deno.land/std/ or
https://deno.land/std/version.ts.

We strongly suggest to always use imports with pinned version of standard
library to avoid unintended changes.

## Troubleshooting

Some of the modules provided in standard library use unstable Deno APIs.

Trying to run such modules without `--unstable` CLI flag ends up with a lot of
TypeScript errors suggesting that some APIs on `Deno` namespace do not exist:

```typescript
// main.ts
import { copy } from "https://deno.land/std@0.50.0/fs/copy.ts";

copy("log.txt", "log-old.txt");
```

```shell
$ deno run --allow-read --allow-write main.ts
Compile file:///dev/deno/main.ts
Download https://deno.land/std@0.50.0/fs/copy.ts
Download https://deno.land/std@0.50.0/fs/ensure_dir.ts
Download https://deno.land/std@0.50.0/fs/_util.ts
error: TS2339 [ERROR]: Property 'utime' does not exist on type 'typeof Deno'.
    await Deno.utime(dest, statInfo.atime, statInfo.mtime);
               ~~~~~
    at https://deno.land/std@0.50.0/fs/copy.ts:90:16

TS2339 [ERROR]: Property 'utimeSync' does not exist on type 'typeof Deno'.
    Deno.utimeSync(dest, statInfo.atime, statInfo.mtime);
         ~~~~~~~~~
    at https://deno.land/std@0.50.0/fs/copy.ts:101:10
```

Solution to that problem requires adding `--unstable` flag:

```shell
deno run --allow-read --allow-write --unstable main.ts
```

To make sure that API producing error is unstable check
[`lib.deno.unstable.d.ts`](https://github.com/denoland/deno/blob/master/cli/js/lib.deno.unstable.d.ts)
declaration.

This problem should be fixed in the near future.
