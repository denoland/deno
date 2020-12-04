## Compiling Executables

> Since the compile functionality is relatively new, the `--unstable` flag has
> to be set in order for the command to work.

`deno compile [SRC] [OUT]` will compile the script into a self contained
executable.

```
> deno compile --unstable https://deno.land/std/http/file_server.ts
```

If you omit the `OUT` parameter, the name of the executable file will be
inferred.

### Cross Compilation

Cross compiling binaries for different platforms is not currently possible.
