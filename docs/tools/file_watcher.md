## File watcher

Deno ships with a built in file watcher that works with other subcommands.

```shell
# watch current directory recursively
deno run --watch server.ts

# watch only server/ directory
deno run --watch=server/ server/mod.ts
```

### Supported subcommands

File watcher can be used with following subcommands:

- `deno bundle`
- `deno lint`
- `deno run`
- `deno test`
