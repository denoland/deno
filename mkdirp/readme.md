# deno-mkdirp

`mkdir -p` 4 `deno`.

## Import

```ts
import { mkdirp } from "https://deno.land/x/std/mkdirp/mkdirp.ts";
```

## API

Same as [`deno.mkdir`](https://deno.land/typedoc/index.html#mkdir).

### `mkdirp(path: string, mode?: number) : Promise<void>`

Creates directories if they do not already exist and makes parent directories as needed.
