// Copyright 2018-2026 the Deno authors. MIT license.

// Polyfill for `process.binding('js_stream').JSStream`.
//
// Node's `JSStream` (src/js_stream.cc) is an internal stream-handle
// constructor used by `tls.connect({ socket })` and similar entry
// points to wrap a JS-side duplex into a libuv-shaped handle. The
// real JS-stream wrapping in this codebase happens via
// `internal/js_stream_socket.js` (see denoland/orchid#90); this
// polyfill only exists so that fixtures probing the C++ binding
// surface (e.g. `parallel/test-js-stream-call-properties.js`,
// `parallel/test-util-types.js`) can construct a `JSStream`
// instance and inspect its shape.

(function () {
const { core } = globalThis.__bootstrap;
const { op_node_make_external } = core.ops;

class JSStream {
  onread: ((...args: unknown[]) => unknown) | null = null;
  onwrite: ((...args: unknown[]) => unknown) | null = null;
  onshutdown: ((...args: unknown[]) => unknown) | null = null;

  _externalStream: unknown;

  constructor() {
    this._externalStream = op_node_make_external();
  }

  readStart(): number {
    return 0;
  }

  readStop(): number {
    return 0;
  }

  doShutdown(req: { oncomplete?: (status: number) => void }): number {
    if (req && typeof req.oncomplete === "function") {
      req.oncomplete(0);
    }
    return 0;
  }

  doWrite(
    req: { oncomplete?: (status: number) => void },
    _bufs: unknown,
    _allBuffers: boolean,
  ): number {
    if (req && typeof req.oncomplete === "function") {
      req.oncomplete(0);
    }
    return 0;
  }

  isAlive(): boolean {
    return true;
  }

  isClosing(): boolean {
    return false;
  }

  finishWrite(
    req: { oncomplete?: (status: number) => void } | null,
    status: number,
  ): void {
    if (req && typeof req.oncomplete === "function") {
      req.oncomplete(status | 0);
    }
  }

  finishShutdown(
    req: { oncomplete?: (status: number) => void } | null,
    status: number,
  ): void {
    if (req && typeof req.oncomplete === "function") {
      req.oncomplete(status | 0);
    }
  }
}

return { JSStream };
})();
