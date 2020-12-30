## Compiling Executables

`deno compile [SRC] [OUT]` will compile the script into a self contained
executable.

```
> deno compile https://deno.land/std/http/file_server.ts
```

If you omit the `OUT` parameter, the name of the executable file will be
inferred.

### Cross Compilation

Cross compiling binaries for different platforms is not currently possible.
