# Deno Docs

## Install

Deno works on OSX, Linux, and Windows. We provide binary download scripts:

With Python:

```
curl -sSf https://raw.githubusercontent.com/denoland/deno_install/master/install.py | python
```

See also [deno_install](https://github.com/denoland/deno_install).

With PowerShell:

```powershell
iex (iwr https://raw.githubusercontent.com/denoland/deno_install/master/install.ps1)
```

_Note: Depending on your security settings, you may have to run
`Set-ExecutionPolicy RemoteSigned -Scope CurrentUser` first to allow downloaded
scripts to be executed._

Try it:

```
> deno http://deno.land/thumb.ts
```

## API Reference

To get an exact reference of deno's runtime API, run the following in the
command line:

```
deno --types
```

In case you don't have it installed yet, but are curious, here is an out-of-date
copy of the output: https://gist.github.com/78855aeeaddeef7c1fce0aeb8ffef8b2

(We do not yet have an HTML version of this. See
https://github.com/denoland/deno/issues/573)

## Examples

### Example: An implementation of the unix "cat" program

The copy here is actually zero-copy. That is, it reads data from the socket and
writes it back to it without ever calling a memcpy() or similar.

```ts
import * as deno from "deno";

for (let i = 1; i < deno.args.length; i++) {
  let filename = deno.args[i];
  let file = await deno.open(filename);
  await deno.copy(deno.stdout, file);
}
```

### Example: A TCP Server echo server

The copy here is actually zero-copy. That is, it reads data from the socket and
writes it back to it without ever calling a memcpy() or similar.

```ts
import { listen, copy } from "deno";
const listener = listen("tcp", ":8080");
while (true) {
  const conn = await listener.accept();
  copy(conn, conn);
}
// TODO top level await doesn't work yet.
```

### Example: Url imports

```ts
import { printHello } from "https://raw.githubusercontent.com/denoland/deno/master/tests/subdir/print_hello.ts";
printHello();
```

The next time you import the same file from same uri it will use the cached
resource instead of downloading it again.

## How to Profile Deno

```sh
# Make sure we're only building release.
export DENO_BUILD_MODE=release
# Build deno and V8's d8.
./tools/build.py d8 deno
# Start the program we want to benchmark with --prof
./target/release/deno tests/http_bench.ts --allow-net --prof &
# Exercise it.
third_party/wrk/linux/wrk http://localhost:4500/
kill `pgrep deno`
# When supplying --prof, V8 will write a file in the current directory that
# looks like this isolate-0x7fad98242400-v8.log
# To examine this file:
D8_PATH=target/release/ ./third_party/v8/tools/linux-tick-processor
isolate-0x7fad98242400-v8.log
```

## Build Instructions _(for advanced users only)_

### Prerequisites:

To ensure reproducible builds, Deno has most of its dependencies in a git
submodule. However, you need to install separately:

1. [Rust](https://www.rust-lang.org/en-US/install.html)
2. [Node](http://nodejs.org/)
3. Python 2.
   [Not 3](https://github.com/denoland/deno/issues/464#issuecomment-411795578).
4. [ccache](https://developer.mozilla.org/en-US/docs/Mozilla/Developer_guide/Build_Instructions/ccache)
   (Optional but helpful for speeding up rebuilds of V8.)
5. Extra steps for Windows users:
   1. Add `python.exe` to `PATH` (e.g. `set PATH=%PATH%;C:\Python27\python.exe`)
   2. Get [VS Community 2017](https://www.visualstudio.com/downloads/). Make
      sure to select the option to install C++ tools and the Windows SDK.
   3. Enable `Debugging Tools for Windows`. Go to `Control Panel` ->
      `Windows 10 SDK` -> Right-Click -> `Change` -> `Change` ->
      `Check Debugging Tools for Windows` -> `Change` -> `Finish`.

### Build:

    # Fetch deps.
    git clone --recurse-submodules https://github.com/denoland/deno.git
    cd deno
    ./tools/setup.py

    # Build.
    ./tools/build.py

    # Run.
    ./target/debug/deno tests/002_hello.ts

    # Test.
    ./tools/test.py

    # Format code.
    ./tools/format.py

Other useful commands:

    # Call ninja manually.
    ./third_party/depot_tools/ninja -C target/debug

    # Build a release binary.
    DENO_BUILD_MODE=release ./tools/build.py :deno

    # List executable targets.
    ./third_party/depot_tools/gn ls target/debug //:* --as=output --type=executable

    # List build configuation.
    ./third_party/depot_tools/gn args target/debug/ --list

    # Edit build configuration.
    ./third_party/depot_tools/gn args target/debug/

    # Describe a target.
    ./third_party/depot_tools/gn desc target/debug/ :deno
    ./third_party/depot_tools/gn help

    # Update third_party modules
    git submodule update

Environment variables: `DENO_BUILD_MODE`, `DENO_BUILD_PATH`, `DENO_BUILD_ARGS`,
`DENO_DIR`.

## Internals

### Internal: libdeno API.

Deno's privileged side will primarily be programmed in Rust. However there will
be a small C API that wraps V8 to 1) define the low-level message passing
semantics, 2) provide a low-level test target, 3) provide an ANSI C API binding
interface for Rust. V8 plus this C API is called "libdeno" and the important
bits of the API is specified here:
https://github.com/denoland/deno/blob/master/libdeno/deno.h
https://github.com/denoland/deno/blob/master/js/libdeno.ts

### Internal: Flatbuffers provide shared data between Rust and V8

We use Flatbuffers to define common structs and enums between TypeScript and
Rust. These common data structures are defined in
https://github.com/denoland/deno/blob/master/src/msg.fbs

## Contributing

See
[CONTRIBUTING.md](https://github.com/denoland/deno/blob/master/.github/CONTRIBUTING.md).

## Change Log

### 2018.10.18 / v0.1.8 / Connecting to Tokio / Fleshing out APIs

Most file system ops were implemented. Basic TCP networking is implemented.
Basic stdio streams exposed. And many random OS facilities were exposed (e.g.
environmental variables)

Tokio was chosen as the backing event loop library. A careful mapping of JS
Promises onto Rust Futures was made, preserving error handling and the ability
to execute synchronously in the main thread.

Continuous benchmarks were added: https://denoland.github.io/deno/ Performance
issues are beginning to be addressed.

"deno --types" was added to reference runtime APIs.

Working towards https://github.com/denoland/deno/milestone/2 We expect v0.2 to
be released in last October or early November.

### 2018.09.09 / v0.1.3 / Scale binding infrastructure

ETA v.0.2 October 2018 https://github.com/denoland/deno/milestone/2

We decided to use Tokio https://tokio.rs/ to provide asynchronous I/O, thread
pool execution, and as a base for high level support for various internet
protocols like HTTP. Tokio is strongly designed around the idea of Futures -
which map quite well onto JavaScript promises. We want to make it as easy as
possible to start a Tokio future from JavaScript and get a Promise for handling
it. We expect this to result in preliminary file system operations, fetch() for
http. Additionally we are working on CI, release, and benchmarking
infrastructure to scale development.

### 2018.08.23 / v0.1.0 / Rust rewrite / V8 snapshot

https://github.com/denoland/deno/commit/68d388229ea6ada339d68eb3d67feaff7a31ca97

Complete! https://github.com/denoland/deno/milestone/1

Go is a garbage collected language and we are worried that combining it with
V8's GC will lead to difficult contention problems down the road.

The V8Worker2 binding/concept is being ported to a new C++ library called
libdeno. libdeno will include the entire JS runtime as a V8 snapshot. It still
follows the message passing paradigm. Rust will be bound to this library to
implement the privileged part of Deno. See deno2/README.md for more details.

V8 Snapshots allow Deno to avoid recompiling the TypeScript compiler at startup.
This is already working.

When the rewrite is at feature parity with the Go prototype, we will release
binaries for people to try.

### 2018.09.32 / v0.0.0 / Golang Prototype / JSConf talk

https://github.com/denoland/deno/tree/golang

https://www.youtube.com/watch?v=M3BM9TB-8yA

http://tinyclouds.org/jsconf2018.pdf

### 2007-2017 / Prehistory

https://github.com/ry/v8worker

http://libuv.org/

http://tinyclouds.org/iocp-links.html

https://nodejs.org/

https://github.com/nodejs/http-parser

http://tinyclouds.org/libebb/

https://en.wikipedia.org/wiki/Merb
