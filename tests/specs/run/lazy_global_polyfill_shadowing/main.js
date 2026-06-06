// Regression test for https://github.com/denoland/deno/issues/34403
// Lazy-loaded globals (Response, Request, ReadableStream, WebSocket, ...) used
// to be writable data properties. After they were converted to accessor
// properties for lazy loading, assigning through an object whose prototype is
// globalThis (the cross-fetch / whatwg-fetch polyfill pattern) would clobber
// the global instead of shadowing on the receiver.

const originalResponse = globalThis.Response;
const originalRequest = globalThis.Request;
const originalReadableStream = globalThis.ReadableStream;
const originalWebSocket = globalThis.WebSocket;

// Mimic the cross-fetch / whatwg-fetch polyfill pattern.
function Self() {}
Self.prototype = globalThis;
const g = new Self();
g.Response = class PolyfillResponse {};
g.Request = class PolyfillRequest {};
g.ReadableStream = class PolyfillReadableStream {};
g.WebSocket = class PolyfillWebSocket {};

console.log(
  "global Response untouched:",
  globalThis.Response === originalResponse,
);
console.log(
  "global Request untouched:",
  globalThis.Request === originalRequest,
);
console.log(
  "global ReadableStream untouched:",
  globalThis.ReadableStream === originalReadableStream,
);
console.log(
  "global WebSocket untouched:",
  globalThis.WebSocket === originalWebSocket,
);

console.log(
  "own Response on receiver:",
  Object.prototype.hasOwnProperty.call(g, "Response"),
);
console.log(
  "own Request on receiver:",
  Object.prototype.hasOwnProperty.call(g, "Request"),
);

// The shadowing own property on the inheriting receiver should be a normal
// enumerable, writable, configurable data property - matching standard
// [[Set]] semantics for an inherited writable data property.
const receiverDesc = Object.getOwnPropertyDescriptor(g, "Response");
console.log(
  "receiver Response descriptor:",
  JSON.stringify({
    hasValue: Object.hasOwn(receiverDesc, "value"),
    writable: receiverDesc.writable,
    enumerable: receiverDesc.enumerable,
    configurable: receiverDesc.configurable,
  }),
);

// `new Response()` still produces a real Response (instanceof works).
const r = new Response("hello");
console.log("real Response instanceof Response:", r instanceof Response);

// Direct assignment to globalThis still works, and preserves the original
// non-enumerable attribute of the global descriptor.
globalThis.WebSocket = "patched";
console.log("direct set on globalThis:", globalThis.WebSocket === "patched");
const globalDesc = Object.getOwnPropertyDescriptor(globalThis, "WebSocket");
console.log(
  "global WebSocket descriptor:",
  JSON.stringify({
    hasValue: Object.hasOwn(globalDesc, "value"),
    writable: globalDesc.writable,
    enumerable: globalDesc.enumerable,
    configurable: globalDesc.configurable,
  }),
);
globalThis.WebSocket = originalWebSocket;
