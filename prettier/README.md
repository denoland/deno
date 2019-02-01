# prettier

Prettier APIs and tools for deno

## Use as a CLI

To formats the source files, run:

```console
deno --allow-run --allow-write https://deno.land/x/std/prettier/main.ts
```

You can format only specific files by passing the arguments.

```console
deno --allow-run --allow-write https://deno.land/x/std/prettier/main.ts path/to/script.ts
```

You can format files on specific directory by passing the directory's path.

```console
deno --allow-run --allow-write https://deno.land/x/std/prettier/main.ts path/to/script.ts
```

## Use API

You can use APIs of prettier as the following:

```ts
import {
  prettier,
  prettierPlugins
} from "https://deno.land/x/std/prettier/prettier.ts";

prettier.format("const x = 1", {
  parser: "babel",
  plugins: prettierPlugins
}); // => "const x = 1;"
```
