## Permissions

<!-- TODO(lucacasonato): what are permissions -->

<!-- TODO(lucacasonato): description of all permissions -->

### Permissions whitelist

Deno also allows you to control the granularity of permissions with whitelists.

This example restricts file system access by whitelisting only the `/usr`
directory:

```shell
$ deno run --allow-read=/usr https://deno.land/std/examples/cat.ts /etc/passwd
error: Uncaught PermissionDenied: read access to "/etc/passwd", run again with the --allow-read flag
► $deno$/dispatch_json.ts:40:11
    at DenoError ($deno$/errors.ts:20:5)
    ...
```

Try it out again with the correct permissions by whitelisting `/etc` instead:

```shell
$ deno run --allow-read=/etc https://deno.land/std/examples/cat.ts /etc/passwd
```

`--allow-write` works the same as `--allow-read`.

This example restricts network access by whitelisting the allowed hosts:

```ts
const result = await fetch("https://deno.land/");
```

```shell
$ deno run --allow-net=deno.land https://deno.land/std/examples/curl.ts https://deno.land/
```
