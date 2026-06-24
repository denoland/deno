// Copyright 2018-2026 the Deno authors. MIT license.

// Mirrors WPT workers/Worker-structure-message: on a single request the worker
// synchronously posts *two* messages back to the host. The host re-arms its
// `onmessage` handler between them (in a `.then`), so the receive loop must run
// a microtask checkpoint between the two already-queued messages or the second
// one is delivered to the stale handler and lost.
self.onmessage = (e: MessageEvent) => {
  if (
    e.data?.operation === "find-edges" && e.data.input instanceof ArrayBuffer
  ) {
    self.postMessage("PASS");
    self.postMessage({
      operation: e.data.operation,
      input: e.data.input,
      threshold: e.data.threshold,
    });
  } else {
    self.postMessage("FAIL");
  }
};
