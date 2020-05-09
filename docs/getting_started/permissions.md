## Permissions

<!-- TODO(lucacasonato): what are permissions -->

<!-- TODO(lucacasonato): description of all permissions -->

### Permissions whitelist

Deno also provides permissions whitelist.

This is an example to restrict file system access by whitelist.

```shell
$ deno run --allow-read=/usr https://deno.land/std/examples/cat.ts /etc/passwd
error: Uncaught PermissionDenied: read access to "/etc/passwd", run again with the --allow-read flag
► $deno$/dispatch_json.ts:40:11
    at DenoError ($deno$/errors.ts:20:5)
    ...
```

You can grant read permission under `/etc` dir

```shell
$ deno run --allow-read=/etc https://deno.land/std/examples/cat.ts /etc/passwd
```

`--allow-write` works same as `--allow-read`.

This is an example to restrict host.

```ts
const result = await fetch("https://deno.land/");
```

```shell
$ deno run --allow-net=deno.land https://deno.land/std/examples/curl.ts https://deno.land/
```
