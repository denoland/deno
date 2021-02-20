## Import maps

> This is an unstable feature. Learn more about
> [unstable features](../runtime/stability.md).

Deno supports [import maps](https://github.com/WICG/import-maps).

You can use import maps with the `--import-map=<FILE>` CLI flag.

Current limitations:

- single import map.
- no fallback URLs.
- Deno does not support `std:` namespace.
- supports only `file:`, `http:` and `https:` schemes.

Example:

**import_map.json**

```js
{
   "imports": {
      "fmt/": "https://deno.land/std@$STD_VERSION/fmt/"
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
$ deno run --import-map=import_map.json --unstable color.ts
```

To use starting directory for absolute imports:

**import_map.json**

```jsonc
{
  "imports": {
    "/": "./"
  }
}
```

**main.ts**

```ts
import { MyUtil } from "/util.ts";
```

You may map a different directory: (eg. src)

**import_map.json**

```jsonc
{
  "imports": {
    "/": "./src/"
  }
}
```
