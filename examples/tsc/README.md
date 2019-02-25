# examples/tsc

This is an example project that build a binary of the TypeScript compiler
(`tsc`) using Deno's underpinnings at the compiler host.

To build, from the root of the Deno project:

```
$ ./tools/build.py :tsc
```

_NOTE_ Currently, Deno core is under development which means a lot of the Deno
infrastructure not use by this example is dragged in. It also means that build
cannot be done by itself and a full build has to be done at least once. So to
build from scratch:

```
$ ./tools/setup.py
$ ./tools/build.py
$ ./tools/build.py :tsc
```
