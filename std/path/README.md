# Deno Path Manipulation Libraries

Usage:

```ts
import * as path from "https://deno.land/std/path/mod.ts";
```

### globToRegExp

Generate a regex based on glob pattern and options This was meant to be using
the the `fs.walk` function but can be used anywhere else.

```ts
import { globToRegExp } from "https://deno.land/std/path/glob.ts";

globToRegExp("foo/**/*.json", {
  flags: "g",
  extended: true,
  globstar: true,
}); // returns the regex to find all .json files in the folder foo
```
