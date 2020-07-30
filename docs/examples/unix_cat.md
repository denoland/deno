## An implementation of the unix "cat" program

In this program each command-line argument is assumed to be a filename, the file
is opened, and printed to stdout.

```ts
for (let i = 0; i < Deno.args.length; i++) {
  const filename = Deno.args[i];
  const file = await Deno.open(filename);
  await Deno.copy(file, Deno.stdout);
  file.close();
}
```

The `copy()` function here actually makes no more than the necessary kernel ->
userspace -> kernel copies. That is, the same memory from which data is read
from the file, is written to stdout. This illustrates a general design goal for
I/O streams in Deno.

Try the program:

```shell
deno run --allow-read https://deno.land/std@$STD_VERSION/examples/cat.ts /etc/passwd
```
