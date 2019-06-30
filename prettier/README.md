# prettier

Prettier APIs and tools for deno

## Use as a CLI

To formats the source files, run:

```bash
deno --allow-read --allow-write https://deno.land/std/prettier/main.ts
```

You can format only specific files by passing the arguments.

```bash
deno --allow-read --allow-write https://deno.land/std/prettier/main.ts path/to/script.ts
```

You can format files on specific directory by passing the directory's path.

```bash
deno --allow-read --allow-write https://deno.land/std/prettier/main.ts path/to/script.ts
```

You can format the input plain text stream. default parse it as typescript code.

```bash
cat path/to/script.ts | deno https://deno.land/std/prettier/main.ts
cat path/to/script.js | deno https://deno.land/std/prettier/main.ts --stdin-parser=babel
cat path/to/config.json | deno https://deno.land/std/prettier/main.ts --stdin-parser=json
cat path/to/README.md | deno https://deno.land/std/prettier/main.ts --stdin-parser=markdown
```

## Use API

You can use APIs of prettier as the following:

```ts
import {
  prettier,
  prettierPlugins
} from "https://deno.land/std/prettier/prettier.ts";

prettier.format("const x = 1", {
  parser: "babel",
  plugins: prettierPlugins
}); // => "const x = 1;"
```
