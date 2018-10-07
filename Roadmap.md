# Deno Roadmap

API and Feature requests should be submitted as PRs to this document.

## Target Use Cases

### Implementation of `cat`

#721

```ts
import * as deno from "deno";

for (let i = 1; i < deno.argv.length; i++) {
  let filename = deno.argv[i];
  let file = await deno.open(filename);
  await deno.copy(deno.stdout, file);
}
```

### TCP Server

#725

```ts
import * as deno from "deno";
const listener = deno.listen("tcp", ":8080");
for await (const conn of listener.accept()) {
  deno.copy(conn, conn);
}
```

### List deps (implemented)

```
% deno --deps http://gist.com/blah.js
http://gist.com/blah.js
http://gist.com/dep.js
https://github.com/denoland/deno/master/testing.js
%
```

## Security Model (partially implemented)

- We want to be secure by default; user should be able to run untrusted code,
  like the web.
- Threat model:
  - Modifiying/deleting local files
  - Leaking private information
- Disallowed default:
  - Network access
  - Local write access
  - Non-JS extensions
  - Subprocesses
  - Env access
- Allowed default:
  - Local read access.
  - argv, stdout, stderr, stdin access always allowed.
  - Maybe: temp dir write access. (But what if they create symlinks there?)
- The user gets prompted when the software tries to do something it doesn't have
  the privilege for.
- Have an option to get a stack trace when access is requested.
- Worried that granting access per file will give a false sense of security due
  to monkey patching techniques. Access should be granted per program (js
  context).

Example security prompts. Options are: YES, NO, PRINT STACK

```
Program requests write access to "~/.ssh/id_rsa". Grant? [yNs]
http://gist.github.com/asdfasd.js requests network access to "www.facebook.com". Grant? [yNs]
Program requests access to environment variables. Grant? [yNs]
Program requests to spawn `rm -rf /`. Grant? [yNs]
```

- cli flags to grant access ahead of time --allow-all --allow-write --allow-net
  --allow-env --allow-exec
- in version two we will add ability to give finer grain access
  --allow-net=facebook.com

## Milestone 1: Rust rewrite / V8 snapshot

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

## Milestone 2: Scale binding infrastructure

ETA: October 2018 https://github.com/denoland/deno/milestone/2

We decided to use Tokio https://tokio.rs/ to provide asynchronous I/O, thread
pool execution, and as a base for high level support for various internet
protocols like HTTP. Tokio is strongly designed around the idea of Futures -
which map quite well onto JavaScript promises. We want to make it as easy as
possible to start a Tokio future from JavaScript and get a Promise for handling
it. We expect this to result in preliminary file system operations, fetch() for
http. Additionally we are working on CI, release, and benchmarking
infrastructure to scale development.

## libdeno C API.

Deno's privileged side will primarily be programmed in Rust. However there will
be a small C API that wraps V8 to 1) define the low-level message passing
semantics 2) provide a low-level test target 3) provide an ANSI C API binding
interface for Rust. V8 plus this C API is called libdeno and the important bits
of the API is specified here:

```c
// Data that gets transmitted.
typedef struct {
  const char* data;
  size_t len;
} deno_buf;

typedef void (*deno_sub_cb)(Deno* d, deno_buf bufs[], size_t nbufs)
void deno_set_callback(Deno* deno, deno_sub_cb cb);

// Executes javascript source code.
// Get error text with deno_last_exception().
// 0 = success, non-zero = failure.
// TODO(ry) Currently the return code has opposite semantics.
int deno_execute(Deno* d, void* user_data, const char* js_filename, const char* js_source);

// This call doesn't go into JS. This is thread-safe.
// TODO(ry) Currently this is called deno_pub. It should be renamed.
// deno_append is the desired name.
void deno_append(deno_buf buf);

// Should only be called at most once during the deno_sub_cb.
void deno_set_response(Deno* deno, deno_buf bufs[], size_t nbufs);

const char* deno_last_exception(Deno* d);
```

## TypeScript API.

This section will not attempt to over all of the APIs but give a general sense
of them.

### Internal: libdeno

This is the lowest-level interface to the privileged side. It provides little
more than passing ArrayBuffers in and out of the VM. The libdeno API is more or
less feature complete now. See
https://github.com/denoland/deno/blob/master/js/libdeno.ts

### Internal: Shared data between Rust and V8

We use Flatbuffers to define common structs and enums between TypeScript and
Rust. These common data structures are defined in
https://github.com/denoland/deno/blob/master/src/msg.fbs This is more or less
working.

### Public API

This is the global variables and various built-in modules, namely the `"deno"`
module.

Deno will provide common browser global utilities like `fetch()` and
`setTimeout()`.

Deno has typescript built-in. Users can access the built-in typescript using:

```ts
import * as ts from "typescript";
```

Deno has its own built-in module which is imported with:

```ts
import * as deno from "deno";
```

The rest of this section discusses what will be in the `deno` module.

Within Deno this is the high-level user facing API. However, the intention is to
expose functionality as simply as possible. There should be little or no
"ergonomics" APIs. (For example, `deno.readFileSync` only deals with
ArrayBuffers and does not have an encoding parameter to return strings.) The
intention is to make very easy to extend and link in external modules which can
then add this functionality.

Deno does not aim to be API compatible with Node in any respect. Deno will
export a single flat namespace "deno" under which all core functions are
defined. We leave it up to users to wrap Deno's namespace to provide some
compatibility with Node.

#### Top-level Await (Not Implemented)

#471

This will be put off until at least deno2 Milestone1 is complete. One of the
major problems is that top-level await calls are not syntactically valid
TypeScript.

#### I/O (Not Implemented) #721

There are many OS constructs that perform I/O: files, sockets, pipes. Deno aims
to provide a unified lowest common denominator interface to work with these
objects. Deno needs to operate on all of these asynchronously in order to not
block the event loop and it.

Sockets and pipes support non-blocking reads and write. Generally file I/O is
blocking but it can be done in a thread pool to avoid blocking the main thread.
Although file I/O can be made asynchronous, it does not support the same
non-blocking reads and writes that sockets and pipes do.

The following interfaces support files, socket, and pipes and are heavily
inspired by Go. The main difference in porting to JavaScript is that errors will
be handled by exceptions, modulo EOF, which is returned as part of `ReadResult`.

```ts
// The bytes read during an I/O call and a boolean indicating EOF.
interface ReadResult {
  nread: number;
  eof: boolean;
}

// Reader is the interface that wraps the basic read() method.
// https://golang.org/pkg/io/#Reader
interface Reader {
  // read() reads up to p.byteLength bytes into p. It returns the number of bytes
  // read (0 <= n <= p.byteLength) and any error encountered. Even if read()
  // returns n < p.byteLength, it may use all of p as scratch space during the
  // call. If some data is available but not p.byteLength bytes, read()
  // conventionally returns what is available instead of waiting for more.
  //
  // When read() encounters an error or end-of-file condition after successfully
  // reading n > 0 bytes, it returns the number of bytes read. It may return the
  // (non-nil) error from the same call or return the error (and n == 0) from a
  // subsequent call. An instance of this general case is that a Reader
  // returning a non-zero number of bytes at the end of the input stream may
  // return either err == EOF or err == nil. The next read() should return 0, EOF.
  //
  // Callers should always process the n > 0 bytes returned before considering
  // the error err. Doing so correctly handles I/O errors that happen after
  // reading some bytes and also both of the allowed EOF behaviors.
  //
  // Implementations of read() are discouraged from returning a zero byte count
  // with a nil error, except when p.byteLength == 0. Callers should treat a
  // return of 0 and nil as indicating that nothing happened; in particular it
  // does not indicate EOF.
  //
  // Implementations must not retain p.
  read(p: ArrayBufferView): Promise<ReadResult>;
}

// Writer is the interface that wraps the basic write() method.
// https://golang.org/pkg/io/#Writer
interface Writer {
  // write() writes p.byteLength bytes from p to the underlying data stream. It
  // returns the number of bytes written from p (0 <= n <= p.byteLength) and any
  // error encountered that caused the write to stop early. write() must return a
  // non-nil error if it returns n < p.byteLength. write() must not modify the
  // slice data, even temporarily.
  //
  // Implementations must not retain p.
  write(p: ArrayBufferView): Promise<number>;
}

// https://golang.org/pkg/io/#Closer
interface Closer {
  // The behavior of Close after the first call is undefined. Specific
  // implementations may document their own behavior.
  close(): void;
}

// https://golang.org/pkg/io/#Seeker
interface Seeker {
  // Seek sets the offset for the next read() or write() to offset, interpreted
  // according to whence: SeekStart means relative to the start of the file,
  // SeekCurrent means relative to the current offset, and SeekEnd means
  // relative to the end. Seek returns the new offset relative to the start of
  // the file and an error, if any.
  //
  // Seeking to an offset before the start of the file is an error. Seeking to
  // any positive offset is legal, but the behavior of subsequent I/O operations
  // on the underlying object is implementation-dependent.
  seek(offset: number, whence: number): Promise<void>;
}

// https://golang.org/pkg/io/#ReadCloser
interface ReaderCloser extends Reader, Closer {}

// https://golang.org/pkg/io/#WriteCloser
interface WriteCloser extends Writer, Closer {}

// https://golang.org/pkg/io/#ReadSeeker
interface ReadSeeker extends Reader, Seeker {}

// https://golang.org/pkg/io/#WriteSeeker
interface WriteSeeker extends Writer, Seeker {}

// https://golang.org/pkg/io/#ReadWriteCloser
interface ReadWriteCloser extends Reader, Writer, Closer {}

// https://golang.org/pkg/io/#ReadWriteSeeker
interface ReadWriteSeeker extends Reader, Writer, Seeker {}
```

These interfaces are well specified, simple, and have very nice utility
functions that will be easy to port. Some example utilites:

```ts
// copy() copies from src to dst until either EOF is reached on src or an error
// occurs. It returns the number of bytes copied and the first error encountered
// while copying, if any.
//
// Because copy() is defined to read from src until EOF, it does not treat an EOF
// from read() as an error to be reported.
//
// https://golang.org/pkg/io/#Copy
async function copy(dst: Writer, src: Reader): Promise<number> {
  let n = 0;
  const b = new ArrayBufferView(1024);
  let got_eof = false;
  while (got_eof === false) {
     let result = await src.read(b);
     if (result.eof) got_eof = true;
     n += await dst.write(b.subarray(0, result.nread));
  }
  return n;
}

// MultiWriter creates a writer that duplicates its writes to all the provided
// writers, similar to the Unix tee(1) command.
//
// Each write is written to each listed writer, one at a time. If a listed
// writer returns an error, that overall write operation stops and returns the
// error; it does not continue down the list.
//
// https://golang.org/pkg/io/#MultiWriter
function multiWriter(writers: ...Writer): Writer {
  return {
    write: async (p: ArrayBufferView) => Promise<number> {
      let n;
      let nwritten = await Promise.all(writers.map((w) => w.write(p)));
      return nwritten[0];
      // TODO unsure of proper semantics for return value..
   }
  };
}
```

A utility function will be provided to make any `Reader` into an
`AsyncIterator`, which has very similar semanatics.

```ts
function readerIterator(r: deno.Reader): AsyncIterator<ArrayBufferView>;
// Example
for await (let buf of readerIterator(socket)) {
  console.log(`read ${buf.byteLength} from socket`);
}
```
