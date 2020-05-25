## Permissions

Deno is secure by default. Therefore, unless you specifically enable it, a deno
module has no file, network, or environment access for example. Access to
security sensitive areas or functions requires the use of permissions to be
granted to a deno process on the command line.

For the following example, `mod.ts` has been granted read-only access to the
file system. It cannot write to it, or perform any other security sensitive
functions.

```shell
deno run --allow-read mod.ts
```

### Permissions list

The following permissions are available:

- **-A, --allow-all** Allow all permissions. This disables all security.
- **--allow-env** Allow environment access for things like getting and setting
  of environment variables.
- **--allow-hrtime** Allow high resolution time measurement. High resolution
  time can be used in timing attacks and fingerprinting.
- **--allow-net=\<allow-net\>** Allow network access. You can specify an
  optional, comma separated list of domains to provide a whitelist of allowed
  domains.
- **--allow-plugin** Allow loading plugins. Please note that --allow-plugin is
  an unstable feature.
- **--allow-read=\<allow-read\>** Allow file system read access. You can specify
  an optional, comma separated list of directories or files to provide a
  whitelist of allowed file system access.
- **--allow-run** Allow running subprocesses. Be aware that subprocesses are not
  run in a sandbox and therefore do not have the same security restrictions as
  the deno process. Therefore, use with caution.
- **--allow-write=\<allow-write\>** Allow file system write access. You can
  specify an optional, comma separated list of directories or files to provide a
  whitelist of allowed file system access.

### Permissions whitelist

Deno also allows you to control the granularity of some permissions with
whitelists.

This example restricts file system access by whitelisting only the `/usr`
directory, however the execution fails as the process was attempting to access a
file in the `/etc` directory:

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

### Network access:

_fetch.ts_:

```ts
const result = await fetch("https://deno.land/");
```

This is an example on how to whitelist hosts/urls:

```shell
$ deno run --allow-net=github.com,deno.land fetch.ts
```

If `fetch.ts` tries to establish network connections to any other domain, the
process will fail.

Allow net calls to any host/url:

```shell
$ deno run --allow-net fetch.ts
```
