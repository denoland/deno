# Deno Roadmap

API and Feature requests should be submitted as PRs to this document.

## Milestone 1: Rust rewrite / V8 snapshot

ETA: July 2018.

Go is a garbage collected language and we are worried that combining it with
V8's GC will lead to difficult contention problems down the road. This work
is being done in the deno2 sub-directory.

The V8Worker2 binding/concept is being ported to a new C++ library called
libdeno. libdeno will include the entire JS runtime as a V8 snapshot. It still
follows the message passing paradigm. Rust will be bound to this library to
implement the privileged part of Deno. See deno2/README.md for more details.

V8 Snapshots allow Deno to avoid recompiling the TypeScript compiler at
startup. This is already working.

When the rewrite is at feature parity with the Go prototype, we will release
binaries for people to try.


## TypeScript API.


There are three layers of API to consider:
* L1: the low-level message passing API exported by libdeno (L1),
* L2: the protobuf messages used internally (L2),
* L3: the final "deno" namespace exported to users (L3).

### L1

https://github.com/ry/deno/blob/master/deno2/js/deno.d.ts

```
pub(channel: string, msg: ArrayBuffer): null | ArrayBuffer;
```
The only interface to make calls outside of V8. You can send an ArrayBuffer and
synchronously receive an ArrayBuffer back. The channel parameter specifies the
purpose of the message.

```
type MessageCallback = (channel: string, msg: ArrayBuffer) => void;
function sub(cb: MessageCallback): void;
```
A way to set a callback to receive messages asynchronously from the privileged
side. Note that there is no way to respond to incoming async messages.
`sub()` is not strictly necessary to implement deno. All communication could
be done through `pub` if there was a message to poll the event loop. For this
reason we should consider removing `sub`.

```
function print(x: string): void;
```
A way to print to stdout. Although this could be easily implemented thru `pub()`
this is an important debugging tool to avoid intermediate infrastructure.


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
