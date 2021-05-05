# Web Platform APIs

Deno aims to use web platform APIs (like `fetch`) instead of inventing a new
proprietary API where it makes sense. These APIs generally follow the
specifications and should match the implementation in Chrome and Firefox. In
some cases it makes sense to deviate from the spec slightly, because of the
different security model Deno has.

Here is a list of web platform APIs Deno implements:

## `fetch` API

### Overview

The `fetch` API can be used to make HTTP requests. It is implemented as
specified in the [WHATWG `fetch` spec](https://fetch.spec.whatwg.org/).

You can find documentation about this API on
[MDN](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API).

### Spec deviations

- The Deno user agent does not have a cookie jar. As such, the `set-cookie`
  header on a response is not processed, or filtered from the visible response
  headers.
- Deno does not follow the same-origin policy, because the Deno user agent
  currently does not have the concept of origins, and it does not have a cookie
  jar. This means Deno does not need to protect against leaking authenticated
  data cross origin. Because of this Deno does not implement the following
  sections of the WHATWG `fetch` specification:
  - Section `3.1. 'Origin' header`.
  - Section `3.2. CORS protocol`.
  - Section `3.5. CORB`.
  - Section `3.6. 'Cross-Origin-Resource-Policy' header`.
  - `Atomic HTTP redirect handling`.
  - The `opaqueredirect` response type.
- A `fetch` with a `redirect` mode of `manual` will return a `basic` response
  rather than an `opaqueredirect` response.

## `CustomEvent`, `EventTarget` and `EventListener`

### Overview

The DOM Event API can be used to dispatch and listen to events happening in an
application. It is implemented as specified in the
[WHATWG DOM spec](https://dom.spec.whatwg.org/#events).

You can find documentation about this API on
[MDN](https://developer.mozilla.org/en-US/docs/Web/API/EventTarget).

### Spec deviations

- Events do not bubble, because Deno does not have a DOM hierarchy, so there is
  no tree for Events to bubble/capture through.

## Web Worker API

### Overview

The WebWorker API can be used to executing code in a separate thread. It is
implemented as specified in the
[WHATWG HTML spec](https://html.spec.whatwg.org/multipage/workers.html#workers).

You can find documentation about this API on
[MDN](https://developer.mozilla.org/en-US/docs/Web/API/Worker).

### Spec deviations

- Currently creating workers from blob URLs is not supported.
- Currently posted data is serialized to JSON instead of structured cloning.
- Currently object ownership cannot be transferred between workers.

## Other APIs

- [Blob](https://developer.mozilla.org/en-US/docs/Web/API/Blob)
- [Console](https://developer.mozilla.org/en-US/docs/Web/API/Console)
- [FormData](https://developer.mozilla.org/en-US/docs/Web/API/FormData)
- [Performance](https://developer.mozilla.org/en-US/docs/Web/API/Performance)
- [setTimeout, setInterval, clearInterval](https://developer.mozilla.org/en-US/docs/Web/API/setTimeout)
- [Streams API](https://developer.mozilla.org/en-US/docs/Web/API/Streams_API)
- [URL](https://developer.mozilla.org/en-US/docs/Web/API/URL)
- [URLSearchParams](https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams)
- [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)

---

## Typings

The TypeScript definitions for the implemented web APIs can be found in the
[`lib.deno.shared_globals.d.ts`](https://github.com/denoland/deno/blob/$CLI_VERSION/cli/dts/lib.deno.shared_globals.d.ts)
and
[`lib.deno.window.d.ts`](https://github.com/denoland/deno/blob/$CLI_VERSION/cli/dts/lib.deno.window.d.ts)
files.

Definitions that are specific to workers can be found in the
[`lib.deno.worker.d.ts`](https://github.com/denoland/deno/blob/$CLI_VERSION/cli/dts/lib.deno.worker.d.ts)
file.
