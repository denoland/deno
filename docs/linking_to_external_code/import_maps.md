## Import maps

> This is an unstable feature. Learn more about
> [unstable features](../runtime/stability.md).

Deno supports [import maps](https://github.com/WICG/import-maps).

You can use import maps with the `--importmap=<FILE>` CLI flag.

Current limitations:

- single import map
- no fallback URLs
- Deno does not support `std:` namespace
- supports only `file:`, `http:` and `https:` schemes

Example:

**import_map.json**

```js
{
   "imports": {
      "fmt/": "https://deno.land/std@0.55.0/fmt/"
   }
}
```

**color.ts**

```ts
import { red } from "fmt/colors.ts";

console.log(red("hello world"));
```

Then:

```shell
$ deno run --importmap=import_map.json --unstable color.ts
```
