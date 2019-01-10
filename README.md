# deno

|      **Linux & Mac**       |        **Windows**         |
| :------------------------: | :------------------------: |
| [![][tci badge]][tci link] | [![][avy badge]][avy link] |

## A secure JavaScript / TypeScript runtime built on V8

- Supports TypeScript out of the box. Uses a recent version of V8. That is, it's
  very modern JavaScript.

- No `package.json`. No npm. Not explicitly compatible with Node.

- Imports reference source code URLs only.

  ```typescript
  import { test } from "https://unpkg.com/deno_testing@0.0.5/testing.ts";
  import { log } from "./util.ts";
  ```

  Remote code is fetched and cached on first execution, and never updated until
  the code is run with the `--reload` flag. (So, this will still work on an
  airplane. See `~/.deno/src` for details on the cache.)

- File system and network access can be controlled in order to run sandboxed
  code. Defaults to read-only file system access and no network access. Access
  between V8 (unprivileged) and Rust (privileged) is only done via serialized
  messages defined in this
  [flatbuffer](https://github.com/denoland/deno/blob/master/src/msg.fbs). This
  makes it easy to audit. To enable write access explicitly use `--allow-write`
  and `--allow-net` for network access.

- Single executable:

  ```
  > ls -lh target/release/deno
  -rwxr-xr-x  1 rld  staff    48M Aug  2 13:24 target/release/deno
  > otool -L target/release/deno
  target/release/deno:
    /usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1252.50.4)
    /usr/lib/libresolv.9.dylib (compatibility version 1.0.0, current version 1.0.0)
    /System/Library/Frameworks/Security.framework/Versions/A/Security (compatibility version 1.0.0, current version 58286.51.6)
    /usr/lib/libc++.1.dylib (compatibility version 1.0.0, current version 400.9.0)
  >
  ```

- Always dies on uncaught errors.

- [Aims to support top-level `await`.](https://github.com/denoland/deno/issues/471)

- Aims to be browser compatible.

See the website for more info [deno.land](https://deno.land).

## Install

With Python:

```
curl -L https://deno.land/x/install/install.py | python
```

With PowerShell:

```powershell
iex (iwr https://deno.land/x/install/install.ps1)
```

_Note: Depending on your security settings, you may have to run
`Set-ExecutionPolicy RemoteSigned -Scope CurrentUser` first to allow downloaded
scripts to be executed._

With [Scoop](https://scoop.sh/):

```
scoop install deno
```

Try it:

```
> deno https://deno.land/thumb.ts
```

See [deno_install](https://github.com/denoland/deno_install) for more
installation methods..

<!-- prettier-ignore -->
[avy badge]: https://ci.appveyor.com/api/projects/status/yel7wtcqwoy0to8x?branch=master&svg=true
[avy link]: https://ci.appveyor.com/project/deno/deno
[tci badge]: https://travis-ci.com/denoland/deno.svg?branch=master
[tci link]: https://travis-ci.com/denoland/deno
