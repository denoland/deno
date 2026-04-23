# AppImage Type-2 Runtime

Vendored prebuilt ELF runtime stubs from the AppImage project. These are
prepended to the SquashFS payload to form a Type-2 AppImage — when the resulting
AppImage is executed, this runtime mounts the embedded SquashFS (via FUSE /
squashfuse) and execs `AppRun` inside it.

Source: <https://github.com/AppImage/type2-runtime> Release tag: `20251108`
(downloaded 2026-04-21).

| File            | Arch    | SHA-256                                                          |
| --------------- | ------- | ---------------------------------------------------------------- |
| runtime-x86_64  | x86_64  | 2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d |
| runtime-aarch64 | aarch64 | 00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444 |

License: MIT (see upstream `LICENSE` at
<https://github.com/AppImage/type2-runtime/blob/main/LICENSE>).

To refresh, download the matching assets from the same release tag, replace the
files in this directory, and update the SHA-256 table above.
