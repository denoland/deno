## Import maps

> This is an unstable feature. Learn more about
> [unstable features](../../runtime/unstable).

Deno supports [import maps](https://github.com/WICG/import-maps).

You can use import map with the `--importmap=<FILE>` CLI flag.

Current limitations:

- single import map
- no fallback URLs
- Deno does not support `std:` namespace
- supports only `file:`, `http:` and `https:` schemes

Example:

```js
// import_map.json

{
   "imports": {
      "http/": "https://deno.land/std/http/"
   }
}
```

```ts
// hello_server.ts

import { serve } from "http/server.ts";

const body = new TextEncoder().encode("Hello World\n");
for await (const req of serve(":8000")) {
  req.respond({ body });
}
```

```shell
$ deno run --importmap=import_map.json hello_server.ts
```
