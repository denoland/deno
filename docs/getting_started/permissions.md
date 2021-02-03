## Permissions

Deno is secure by default. Therefore, unless you specifically enable it, a deno
module has no file, network, or environment access for example. Access to
security-sensitive areas or functions requires the use of permissions to be
granted to a deno process on the command line.

For the following example, `mod.ts` has been granted read-only access to the
file system. It cannot write to it, or perform any other security-sensitive
functions.

```shell
deno run --allow-read mod.ts
```

### Permissions list

The following permissions are available:

- **-A, --allow-all** Allow all permissions. This disables all security.
- **--allow-env** Allow environment access for things like getting and setting
  of environment variables.
- **--allow-hrtime** Allow high-resolution time measurement. High-resolution
  time can be used in timing attacks and fingerprinting.
- **--allow-net=\<allow-net\>** Allow network access. You can specify an
  optional, comma-separated list of domains to provide an allow-list of allowed
  domains.
- **--allow-plugin** Allow loading plugins. Please note that --allow-plugin is
  an unstable feature.
- **--allow-read=\<allow-read\>** Allow file system read access. You can specify
  an optional, comma-separated list of directories or files to provide a
  allow-list of allowed file system access.
- **--allow-run** Allow running subprocesses. Be aware that subprocesses are not
  run in a sandbox and therefore do not have the same security restrictions as
  the deno process. Therefore, use with caution.
- **--allow-write=\<allow-write\>** Allow file system write access. You can
  specify an optional, comma-separated list of directories or files to provide a
  allow-list of allowed file system access.

### Permissions allow-list

Deno also allows you to control the granularity of some permissions with
allow-lists.

This example restricts file system access by allow-listing only the `/usr`
directory, however the execution fails as the process was attempting to access a
file in the `/etc` directory:

```shell
$ deno run --allow-read=/usr https://deno.land/std@$STD_VERSION/examples/cat.ts /etc/passwd
error: Uncaught PermissionDenied: read access to "/etc/passwd", run again with the --allow-read flag
► $deno$/dispatch_json.ts:40:11
    at DenoError ($deno$/errors.ts:20:5)
    ...
```

Try it out again with the correct permissions by allow-listing `/etc` instead:

```shell
deno run --allow-read=/etc https://deno.land/std@$STD_VERSION/examples/cat.ts /etc/passwd
```

`--allow-write` works the same as `--allow-read`.

### Network access:

_fetch.ts_:

```ts
const result = await fetch("https://deno.land/");
```

This is an example of how to allow-list hosts/urls:

```shell
deno run --allow-net=github.com,deno.land fetch.ts
```

If `fetch.ts` tries to establish network connections to any other domain, the
process will fail.

Allow net calls to any host/url:

```shell
deno run --allow-net fetch.ts
```

### Security Table
This is not a comprehensive list, it mostly exists to help visualize what you are exposing your system to when enabling a given permission.
| Permission       | Allows Read | Allows Write | Allows Net | Allows Arbitrary Code Execution | Notes |
|------------------|-------------|--------------|------------|---------------------------------|-------|
| `-A`             | Entire Disk | Entire Disk  | Entire Net | Yes                             |       |
| `--allow-plugin` | Entire Disk | Entire Disk  | Entire Net | Yes                             | [plugin] |
| `--allow-run`    | Entire Disk | Entire Disk  | Entire Net | Yes                             | [run] |
| `--allow-net`    | No          | No           | Entire Net | Through Web Workers             |       |
| `--allow-env`    | No*         | No*          | No*        | No*                             | [env] |
| `--allow-read`   | Entire Disk | No           | No         | No                              |       |
| `--allow-read=/usr`   | /usr/* | No           | No         | No                              |       |
| `--allow-write`  | No          | Entire Disk  | No         | No                              |       |
| `--allow-write=/usr` | No      | /usr/*       | No         | No                              |       |
| `--allow-net=1.1.1.1,8.8.8.8` | No | No | 1.1.1.1 & 8.8.8.8 | Not unless hosted @ 1.1.1.1 or 8.8.8.8 |  |
| `--allow-hrtime` | No          | No           | No         | No                              | [hrt] |

* [plugin]: Plugins allow for arbitrary shared objects to be loaded into deno.  
* [run]: Run allows for any code to be run, including wget, chmod, and any program downloaded from wget and the execute bit flipped with chmod.  
* [env]: If the environment variables are used in another program after deno it could bypass the sandbox, but that is outside of this scope.  
* [hrt]: As stated above, High-resolution
  time can be used in timing attacks and fingerprinting.

### Conference

Ryan Dahl. (September 25, 2020).
[The Deno security model](https://www.youtube.com/watch?v=r5F6dekUmdE#t=34m57).
Speakeasy JS.
