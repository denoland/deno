# Deno Documentation

## Disclaimer

A word of caution: Deno is very much under development. We encourage brave early
adopters, but expect bugs large and small. The API is subject to change without
notice.

[Bug reports](https://github.com/denoland/deno/issues) do help!

## Install

Deno works on OSX, Linux, and Windows. Deno is a single binary executable. It
has no external dependencies.

[deno_install](https://github.com/denoland/deno_install) provides convenience
scripts to download and install the binary.

Using Python:

```
curl -L https://deno.land/x/install/install.py | python
```

Or using PowerShell:

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

Deno can also be installed manually, by downloading a tarball or zip file at
[github.com/denoland/deno/releases](https://github.com/denoland/deno/releases).
These packages contain just a single executable file. You will have to set the
executable bit on Mac and Linux.

Try it:

```
> deno https://deno.land/thumb.ts
```

## API Reference

To get an exact reference of deno's runtime API, run the following in the
command line:

```
> deno --types
```

Or see the [doc website](https://deno.land/typedoc/index.html).

If you are embedding deno in a Rust program, see
[the rust docs](https://deno.land/rustdoc/deno/index.html).

## Tutorial

### An implementation of the unix "cat" program

In this program each command-line argument is assumed to be a filename, the file
is opened, and printed to stdout.

```ts
import * as deno from "deno";

(async () => {
  for (let i = 1; i < deno.args.length; i++) {
    let filename = deno.args[i];
    let file = await deno.open(filename);
    await deno.copy(deno.stdout, file);
    file.close();
  }
})();
```

The `copy()` function here actually makes no more than the necessary kernel ->
userspace -> kernel copies. That is, the same memory from which data is read
from the file, is written to stdout. This illustrates a general design goal for
I/O streams in Deno.

Try the program:

```
> deno https://deno.land/x/examples/cat.ts /etc/passwd
```

### TCP echo server

This is an example of a simple server which accepts connections on port 8080,
and returns to the client anything it sends.

```ts
import { listen, copy } from "deno";

(async () => {
  const addr = "0.0.0.0:8080";
  const listener = listen("tcp", addr);
  console.log("listening on", addr);
  while (true) {
    const conn = await listener.accept();
    copy(conn, conn);
  }
})();
```

When this program is started, the user is prompted for permission to listen on
the network:

```
> deno https://deno.land/x/examples/echo_server.ts
deno requests network access to "listen". Grant? [yN] y
listening on 0.0.0.0:8080
```

For security reasons, deno does not allow programs to access the network without
explicit permission. To avoid the console prompt, use a command-line flag:

```
> deno https://deno.land/x/examples/echo_server.ts --allow-net
```

To test it, try sending a HTTP request to it by using curl. The request gets
written directly back to the client.

```
> curl http://localhost:8080/
GET / HTTP/1.1
Host: localhost:8080
User-Agent: curl/7.54.0
Accept: */*
```

It's worth noting that like the `cat.ts` example, the `copy()` function here
also does not make unnecessary memory copies. It receives a packet from the
kernel and sends back, without further complexity.

### Linking to third party code

In the above examples, we saw that Deno could execute scripts from URLs. Like
browser JavaScript, Deno can import libraries directly from URLs. This example
uses a URL to import a test runner library:

```ts
import { test, assertEqual } from "https://deno.land/x/testing/mod.ts";

test(function t1() {
  assertEqual("hello", "hello");
});

test(function t2() {
  assertEqual("world", "world");
});
```

Try running this:

```
> deno https://deno.land/x/examples/example_test.ts
Compiling /Users/rld/src/deno_examples/example_test.ts
Downloading https://deno.land/x/testing/mod.ts
Compiling https://deno.land/x/testing/mod.ts
running 2 tests
test t1
... ok
test t2
... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Note that we did not have to provide the `--allow-net` flag for this program,
and yet it accessed the network. The runtime has special access to download
imports and cache them to disk.

Deno caches remote imports in a special directory specified by the `$DENO_DIR`
environmental variable. It default to `$HOME/.deno` if `$DENO_DIR` is not
specified. The next time you run the program, no downloads will be made. If the
program hasn't changed, it won't be recompiled either.

**But what if `https://deno.land/` goes down?** Relying on external servers is
convenient for development but brittle in production. Production software should
always bundle its dependencies. In Deno this is done by checking the `$DENO_DIR`
into your source control system, and specifying that path as the `$DENO_DIR`
environmental variable at runtime.

**How do you import to a specific version?** Simply specify the version in the
URL. For example, this URL fully specifies the code being run:
`https://unpkg.com/liltest@0.0.5/dist/liltest.js`. Combined with the
aforementioned technique of setting `$DENO_DIR` in production to stored code,
one can fully specify the exact code being run, and execute the code without
network access.

**It seems unwieldy to import URLs everywhere. What if one of the URLs links to
a subtly different version of a library? Isn't it error prone to maintain URLs
everywhere in a large project?** The solution is to import and re-export your
external libraries in a central `package.ts` file (which serves the same purpose
as Node's `package.json` file). For example, let's say you were using the above
testing library across a large project. Rather than importing
`"https://deno.land/x/testing/mod.ts"` everywhere, you could create a
`package.ts` file the exports the third-party code:

```ts
export { test, assertEqual } from "https://deno.land/x/testing/mod.ts";
```

And throughout project one can import from the `package.ts` and avoid having
many references to the same URL:

```ts
import { test, assertEqual } from "./package.ts";
```

This design circumvents a plethora of complexity spawned by package management
software, centralized code repositories, and superfluous file formats.

## Useful command line flags

V8 has many many command-line flags, that you can see with `--v8-options`. Here
are a few particularly useful ones:

```
--async-stack-traces
```

## How to Profile deno

To start profiling,

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
```

V8 will write a file in the current directory that looks like this:
`isolate-0x7fad98242400-v8.log`. To examine this file:

```sh
D8_PATH=target/release/ ./third_party/v8/tools/linux-tick-processor
isolate-0x7fad98242400-v8.log > prof.log
# on macOS, use ./third_party/v8/tools/mac-tick-processor instead
```

`prof.log` will contain information about tick distribution of different calls.

To view the log with Web UI, generate JSON file of the log:

```sh
D8_PATH=target/release/ ./third_party/v8/tools/linux-tick-processor
isolate-0x7fad98242400-v8.log --preprocess > prof.json
```

Open `third_party/v8/tools/profview/index.html` in your brower, and select
`prof.json` to view the distribution graphically.

To learn more about `d8` and profiling, check out the following links:

- [https://v8.dev/docs/d8](https://v8.dev/docs/d8)
- [https://v8.dev/docs/profile](https://v8.dev/docs/profile)

## How to Debug deno

We can use LLDB to debug deno.

```sh
lldb -- target/debug/deno tests/worker.js
> run
> bt
> up
> up
> l
```

To debug Rust code, we can use `rust-lldb`. It should come with `rustc` and is a
wrapper around LLDB.

```sh
rust-lldb -- ./target/debug/deno tests/http_bench.ts --allow-net
# On macOS, you might get warnings like
# `ImportError: cannot import name _remove_dead_weakref`
# In that case, use system python by setting PATH, e.g.
# PATH=/System/Library/Frameworks/Python.framework/Versions/2.7/bin:$PATH
(lldb) command script import "/Users/kevinqian/.rustup/toolchains/1.30.0-x86_64-apple-darwin/lib/rustlib/etc/lldb_rust_formatters.py"
(lldb) type summary add --no-value --python-function lldb_rust_formatters.print_val -x ".*" --category Rust
(lldb) type category enable Rust
(lldb) target create "../deno/target/debug/deno"
Current executable set to '../deno/target/debug/deno' (x86_64).
(lldb) settings set -- target.run-args  "tests/http_bench.ts" "--allow-net"
(lldb) b op_start
(lldb) r
```

## Build Instructions _(for advanced users only)_

### Prerequisites:

To ensure reproducible builds, deno has most of its dependencies in a git
submodule. However, you need to install separately:

1. [Rust](https://www.rust-lang.org/en-US/install.html) >= 1.30.0
2. [Node](https://nodejs.org/)
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

    # List build configuration.
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

deno's privileged side will primarily be programmed in Rust. However there will
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

### Internal: Updating prebuilt binaries

```
./third_party/depot_tools/upload_to_google_storage.py -b denoland  \
  -e ~/.config/gcloud/legacy_credentials/ry@tinyclouds.org/.boto `which sccache`
mv `which sccache`.sha1 prebuilt/linux64/
gsutil acl ch -u AllUsers:R gs://denoland/608be47bf01004aa11d4ed06955414e93934516e
```

## Contributing

See
[CONTRIBUTING.md](https://github.com/denoland/deno/blob/master/.github/CONTRIBUTING.md).
