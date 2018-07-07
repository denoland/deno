# deno

[![Build Status](https://travis-ci.com/ry/deno.svg?branch=master)](https://travis-ci.com/ry/deno)

## A secure TypeScript runtime built on V8

* Supports TypeScript 2.8 out of the box. Uses V8 6.8.275.3. That is, it's
  very modern JavaScript.

* No `package.json`. No npm. Not explicitly compatible with Node.

* Imports reference source code URLs only.
	```
  import { test } from "https://unpkg.com/deno_testing@0.0.5/testing.ts"
  import { log } from "./util.ts"
	```
  Remote code is fetched and cached on first execution, and never updated until
  the code is run with the `--reload` flag. (So, this will still work on an
  airplane. See `~/.deno/src` for details on the cache.)

* File system and network access can be controlled in order to run sandboxed
  code. Defaults to read-only file system access and no network access.
	Access between V8 (unprivileged) and Golang (privileged) is only done via
  serialized messages defined in this
  [protobuf](https://github.com/ry/deno/blob/master/src/msg.proto). This makes it
  easy to audit.
	To enable write access explicitly use `--allow-write` and `--allow-net` for
  network access.

* Single executable:
	```
	> ls -lh deno
	-rwxrwxr-x 1 ryan ryan 55M May 28 23:46 deno
	> ldd deno
		linux-vdso.so.1 =>  (0x00007ffc6797a000)
		libpthread.so.0 => /lib/x86_64-linux-gnu/libpthread.so.0 (0x00007f104fa47000)
		libstdc++.so.6 => /usr/lib/x86_64-linux-gnu/libstdc++.so.6 (0x00007f104f6c5000)
		libm.so.6 => /lib/x86_64-linux-gnu/libm.so.6 (0x00007f104f3bc000)
		libgcc_s.so.1 => /lib/x86_64-linux-gnu/libgcc_s.so.1 (0x00007f104f1a6000)
		libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007f104eddc000)
		/lib64/ld-linux-x86-64.so.2 (0x00007f104fc64000)
	```

* Always dies on uncaught errors.

* Supports top-level `await`.

* Aims to be browser compatible.


## Status

Segfaulty. Check back soon.

Roadmap is [here](https://github.com/ry/deno/blob/master/Roadmap.md).

Also see this presentation: http://tinyclouds.org/jsconf2018.pdf

### Github Noise

I am excited about all the interest in this project. However, do understand that this
is very much a non-functional prototype. There's a huge amount of heavy lifting to do.
Unless you are participating in that, please maintain radio silence on github. This
includes submitting trivial PRs (like improving README build instructions).

## Compile instructions

Get [Depot Tools](http://commondatastorage.googleapis.com/chrome-infra-docs/flat/depot_tools/docs/html/depot_tools_tutorial.html#_setting_up) and make sure it's in your path.

You need [yarn](https://yarnpkg.com/lang/en/docs/install/) installed.

You need [rust](https://www.rust-lang.org/en-US/install.html) installed.

You might want  [ccache](https://developer.mozilla.org/en-US/docs/Mozilla/Developer_guide/Build_Instructions/ccache) installed.

Fetch the third party dependencies.

    ./tools/build_third_party.py

Generate ninja files.

    gn gen out/Default
    gn gen out/Release --args='cc_wrapper="ccache" is_official_build=true'
    gn gen out/Debug --args='cc_wrapper="ccache" is_debug=true '

Then build with ninja (will take a while to complete):

    ninja -C out/Debug/ deno

Other useful commands:

    gn args out/Debug/ --list
    gn args out/Debug/
    gn desc out/Debug/ :deno
    gn help

