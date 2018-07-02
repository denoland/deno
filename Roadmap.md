# Deno Roadmap

API and Feature requests should be submitted as PRs to this document.

## Target Use Cases

### Low-level, fast memory efficient sockets

Example, non-final API for piping a socket to stdout:

```javascript
function nonblockingpipe(fd) {
  let buf = new Uint8Array(1024); // Fixed 1k buffer.
  for (;;) {
    let code = await deno.pollNB(fd, deno.POLL_RD | deno.POLL_WR);
    switch (code) {
    case "READABLE":
       let [nread, err] = deno.readNB(fd, buf, buf.byteSize);
       if (err === "EAGAIN") continue;
       if (err != null) break;
       await deno.stdout.write(buf.slice(0, nread));
       break;
    case "ERROR":
       throw Error("blah");
    }
  }
}
```

### List deps

```
% deno --list-deps http://gist.com/blah.js
http://gist.com/blah.js
http://gist.com/dep.js
https://github.com/ry/deno/master/testing.js
%
```

## Security Model

* We want to be secure by default; user should be able to run untrusted code,
  like the web.
* Threat model:
  * Modifiying/deleting local files
  * Leaking private information
* Disallowed default:
    * No network access
    * No local write access
    * No non-js extensions
    * No subprocesses
    * No env access
* Allowed default:
    * Local read access.
    * argv, stdout, stderr, stdin access always allowed.
    * Optional: temp dir by default. (But what if they create symlinks there?)
* The user gets prompted when the software tries to do something it doesn't have
  the privilege for.
* Have an option to get a stack trace when access is requested.
* Worried that granting access per file will give a false sense of security due
  to monkey patching techniques. Access should be granted per program (js
  context).

Example security prompts. Options are: YES, NO, PRINT STACK
```
Program requests write access to "~/.ssh/id_rsa". Grant? [yNs]
http://gist.github.com/asdfasd.js requests network access to "www.facebook.com". Grant? [yNs]
Program requests access to environment variables. Grant? [yNs]
Program requests to spawn `rm -rf /`. Grant? [yNs]
```

* cli flags to grant access ahead of time --allow-all --allow-write --allow-net
  --allow-env --allow-exec
* in version two we will add ability to give finer grain access
  --allow-net=facebook.com

## Milestone 1: Rust rewrite / V8 snapshot

ETA: July 2018.

Go is a garbage collected language and we are worried that combining it with
V8's GC will lead to difficult contention problems down the road.

The V8Worker2 binding/concept is being ported to a new C++ library called
libdeno. libdeno will include the entire JS runtime as a V8 snapshot. It still
follows the message passing paradigm. Rust will be bound to this library to
implement the privileged part of Deno. See deno2/README.md for more details.

V8 Snapshots allow Deno to avoid recompiling the TypeScript compiler at
startup. This is already working.

When the rewrite is at feature parity with the Go prototype, we will release
binaries for people to try.

## libdeno C API.

Deno's privileged side will primarily be programmed in Rust. However there
will be a small C API that wraps V8 to 1) define the low-level message passing
semantics 2) provide a low-level test target 3) provide an ANSI C API binding
interface for Rust. V8 plus this C API is called libdeno and the important bits
of the API is specified here:

```c
// Data that gets transmitted.
typedef struct {
  const char* data;
  size_t len;
} deno_buf;

typedef void (*deno_sub_cb)(Deno* d, const char* channel,
                            deno_buf bufs[], size_t nbufs)
void deno_set_callback(Deno* deno, deno_sub_cb cb);

// Executes javascript source code.
// Get error text with deno_last_exception().
// 0 = success, non-zero = failure.
// TODO(ry) Currently the return code has opposite semantics.
int deno_execute(Deno* d, const char* js_filename, const char* js_source);

// This call doesn't go into JS. This is thread-safe.
// TODO(ry) Currently this is called deno_pub. It should be renamed.
// deno_append is the desired name.
void deno_append(deno_buf buf);

// Should only be called at most once during the deno_sub_cb.
void deno_set_response(Deno* deno, deno_buf bufs[], size_t nbufs);

const char* deno_last_exception(Deno* d);
```

## TypeScript API.


There are three layers of API to consider:
* L1: the low-level message passing API exported by libdeno (L1),
* L2: the protobuf messages used internally (L2),
* L3: the final "deno" namespace exported to users (L3).

### L1

```typescript
function send(channel: string, ...ab: ArrayBuffer[]): ArrayBuffer[] | null;
```
Used to make calls outside of V8. Send an ArrayBuffer and synchronously receive
an ArrayBuffer back. The channel parameter specifies the purpose of the message.

```typescript
function poll(): ArrayBuffer[];
```
Poll for new asynchronous events from the privileged side. This will be done
as the main event loop.

```typescript
function print(x: string): void;
```
A way to print to stdout. Although this could be easily implemented thru
`send()` this is an important debugging tool to avoid intermediate
infrastructure.


The current implementation is out of sync with this document:
https://github.com/ry/deno/blob/master/js/deno.d.ts

#### L1 Examples

The main event loop of Deno should look something like this:
```js
function main() {
   // Setup...
   while (true) {
      const messages = deno.poll();
      processMessages(messages);
   }
}
```


### L2

https://github.com/ry/deno/blob/master/msg.proto

### L3

With in Deno this is the high-level user facing API. However, the intention
is to expose functionality as simply as possible. There should be little or
no "ergonomics" APIs. (For example, `deno.readFileSync` only deals with
ArrayBuffers and does not have an encoding parameter to return strings.)
The intention is to make very easy to extend and link in external modules
which can then add this functionality.

Deno does not aim to be API compatible with Node in any respect. Deno will
export a single flat namespace "deno" under which all core functions are
defined.  We leave it up to users to wrap Deno's namespace to provide some
compatibility with Node.

*Top-level await*: This will be put off until at least deno2 Milestone1 is
complete. One of the major problems is that top-level await calls are not
syntactically valid TypeScript.

Functions exported under Deno namespace:
```ts
deno.readFileSync(filename: string): ArrayBuffer;
deno.writeFileSync(filename: string, data: Uint8Array, perm: number): void;
```

Timers:
```ts
setTimeout(cb: TimerCallback, delay: number, ...args: any[]): number;
setInterval(cb: TimerCallbac, duration: number, ...args: any[]): number;
clearTimeout(timerId: number);
clearInterval(timerId: number);
```

Console:
```ts
declare var console: {
  log(...args: any[]): void;
  error(...args: any[]): void;
  assert(assertion: boolean, ...msg: any[]): void;
}
```

URL:
```ts
URL(url: string, base?: string): URL;
```

Text encoding:
```ts
declare var TextEncoder: {
  new (utfLabel?: string, options?: TextEncoderOptions): TextEncoder;
  (utfLabel?: string, options?: TextEncoderOptions): TextEncoder;
  encoding: string;
};

declare var TextDecoder: {
  new (label?: string, options?: TextDecoderOptions): TextDecoder;
  (label?: string, options?: TextDecoderOptions): TextDecoder;
  encoding: string;
};
```

Fetch API:
```ts
fetch(input?: Request | string, init?: RequestInit): Promise<Response>;
```
