# Runtime JavaScript Code

This directory contains Deno runtime code written in plain JavaScript.

Each file is a plain, old **script**, not ES modules. The reason is that
snapshotting ES modules is much harder, especially if one needs to manipulate
global scope (like in case of Deno).

Each file is prefixed with a number, telling in which order scripts should be
loaded into V8 isolate. This is temporary solution and we're striving not to
require specific order (though it's not 100% obvious if that's feasible).

## Deno Web APIs

This directory facilities Web APIs that are available in Deno.

Please note, that some implementations might not be completely aligned with
specification.

Some Web APIs are using ops under the hood, eg. `console`, `performance`.

## Implemented Web APIs

- [Blob](https://developer.mozilla.org/en-US/docs/Web/API/Blob): for
  representing opaque binary data.
- [Console](https://developer.mozilla.org/en-US/docs/Web/API/Console): for
  logging purposes.
- [CustomEvent](https://developer.mozilla.org/en-US/docs/Web/API/CustomEvent),
  [EventTarget](https://developer.mozilla.org/en-US/docs/Web/API/EventTarget)
  and
  [EventListener](https://developer.mozilla.org/en-US/docs/Web/API/EventListener):
  to work with DOM events.
  - **Implementation notes:** There is no DOM hierarchy in Deno, so there is no
    tree for Events to bubble/capture through.
- [fetch](https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope/fetch),
  [Request](https://developer.mozilla.org/en-US/docs/Web/API/Request),
  [Response](https://developer.mozilla.org/en-US/docs/Web/API/Response),
  [Body](https://developer.mozilla.org/en-US/docs/Web/API/Body) and
  [Headers](https://developer.mozilla.org/en-US/docs/Web/API/Headers): modern
  Promise-based HTTP Request API.
- [location](https://developer.mozilla.org/en-US/docs/Web/API/Window/location)
  and [Location](https://developer.mozilla.org/en-US/docs/Web/API/Location).
- [FormData](https://developer.mozilla.org/en-US/docs/Web/API/FormData): access
  to a `multipart/form-data` serialization.
- [Performance](https://developer.mozilla.org/en-US/docs/Web/API/Performance):
  retrieving current time with a high precision.
- [setTimeout](https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope/setTimeout),
  [setInterval](https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope/setInterval),
  [clearTimeout](https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope/clearTimeout):
  scheduling callbacks in future and
  [clearInterval](https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope/clearInterval).
- [Stream](https://developer.mozilla.org/en-US/docs/Web/API/Streams_API) for
  creating, composing, and consuming streams of data.
- [URL](https://developer.mozilla.org/en-US/docs/Web/API/URL) and
  [URLSearchParams](https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams):
  to construct and parse URLSs.
- [Worker](https://developer.mozilla.org/en-US/docs/Web/API/Worker): executing
  additional code in a separate thread.
  - **Implementation notes:** Blob URLs are not supported, object ownership
    cannot be transferred, posted data is serialized to JSON instead of
    [structured cloning](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Structured_clone_algorithm).
