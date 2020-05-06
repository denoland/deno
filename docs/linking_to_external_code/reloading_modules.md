## Reloading modules

You can invalidate your local `DENO_DIR` cache using the `--reload` flag. It's
usage is described below:

To reload everything

`--reload`

Sometimes we want to upgrade only some modules. You can control it by passing an
argument to a `--reload` flag.

To reload all standard modules

`--reload=https://deno.land/std`

To reload specific modules (in this example - colors and file system utils) use
a comma to separate URLs

`--reload=https://deno.land/std/fs/utils.ts,https://deno.land/std/fmt/colors.ts`

<!-- Should this be part of examples? --
