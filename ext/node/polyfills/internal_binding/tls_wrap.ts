// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any prefer-primordials

import { PipeWrap, TLSWrap } from "ext:core/ops";
import {
  kReadBytesOrError,
  streamBaseState,
} from "ext:deno_node/internal_binding/stream_wrap.ts";
// Use Symbol.for to access symbols from js_stream_socket.js
// without importing it (avoids circular dependency).
const kJSStreamHandle = Symbol.for("kJSStreamHandle");
const kOwner = Symbol.for("kJSStreamOwner");

export { TLSWrap };

/**
 * Create a TLSWrap that intercepts an underlying stream handle.
 * Mirrors Node's `internalBinding('tls_wrap').wrap(handle, context, isServer)`.
 *
 * @param handle - The underlying stream handle (TCP CppGC object or JSStreamSocket handle)
 * @param context - SecureContext object { ca, cert, key, rejectUnauthorized }
 * @param isServer - Whether this is a server-side TLS connection
 * @param servername - SNI hostname for client connections
 */
export function wrap(
  handle: any,
  context: any,
  isServer: boolean,
  servername?: string,
): TLSWrap {
  const kind = isServer ? 1 : 0;
  // TODO(@nathanwhit): use a proper async_id from async_hooks
  const asyncId = 0;
  const res = new TLSWrap(kind, asyncId);

  // initClientTls/initServerTls take the SecureContext JS object directly
  // and build the rustls config from its { ca, cert, key } properties.
  // The actual TLS connection is deferred until start() so that
  // setALPNProtocols can modify the config first.
  const initResult = isServer
    ? res.initServerTls(context)
    : res.initClientTls(servername || "", context);
  if (initResult !== 0) {
    const err = new Error("unsupported protocol");
    // rustls cannot negotiate TLSv1.0/TLSv1.1, so surface the closest
    // OpenSSL-style error instead of hanging the socket.
    (err as Error & { code?: string }).code = "ERR_SSL_UNSUPPORTED_PROTOCOL";
    // Store the init error on the handle so the caller can detect and
    // report it instead of throwing synchronously.  A throw from here
    // surfaces as an uncaught exception when the TLSSocket is created
    // inside a native libuv callback (e.g. server_connection_cb) because
    // there is no native TryCatch scope on the call stack.
    res._initError = err;
    return res;
  }

  const nativeHandle = handle;

  if (nativeHandle[kJSStreamHandle]) {
    // JS-backed stream (e.g. JSStreamSocket wrapping a Duplex).
    // Use attachJsStream instead of attach -I/O goes through JS callbacks.
    const attachResult = res.attachJsStream();
    if (attachResult !== 0) {
      throw new Error(`TLS wrap attach (JS stream) failed: ${attachResult}`);
    }

    // Pull-based encrypted output: instead of Rust calling a JS
    // callback (which causes reentrancy issues), Rust buffers encrypted
    // data and JS drains it after each operation that may produce output.
    // The flush is scheduled via Promise.resolve().then() to avoid
    // synchronous feedback loops through DuplexPair.
    const jsStreamOwner = nativeHandle[kOwner];
    let flushPending = false;
    const flushEncOut = () => {
      if (flushPending) return;
      flushPending = true;
      Promise.resolve().then(() => {
        flushPending = false;
        let data;
        // Drain in a loop - writing to DuplexPair may trigger readBuffer
        // on the other side, which produces more encrypted output.
        while (
          (data = res.drainEncOut()) && data.byteLength > 0 &&
          jsStreamOwner?.stream
        ) {
          jsStreamOwner.stream.write(new Uint8Array(data));
        }
      });
    };

    // Wire up readBuffer/emitEOF: JSStreamSocket calls these when the
    // underlying Duplex stream produces data or ends.
    // After each call, schedule a drain of encrypted output.
    nativeHandle.readBuffer = (chunk: Uint8Array) => {
      res.readBuffer(chunk);
      flushEncOut();
    };
    nativeHandle.emitEOF = () => {
      res.emitEof();
      flushEncOut();
    };

    // Store flush function on the TLSWrap so _start() can drain
    // after initiating the TLS handshake.
    res._flushEncOut = flushEncOut;
  } else {
    // Native stream (TCP or Pipe handle).
    // attach/attachPipe stores the stream pointer for encrypted writes.
    const attachResult = nativeHandle instanceof PipeWrap
      ? res.attachPipe(nativeHandle)
      : res.attach(nativeHandle);
    if (attachResult !== 0) {
      throw new Error(`TLS wrap attach failed: ${attachResult}`);
    }

    // Read interception at the JS layer: intercept the TCPWrap's onread
    // callback to forward encrypted data from the TCP stream to the TLSWrap
    // via receive().
    // Note: LibUvStreamWrap's read callback uses (buf) signature with nread
    // in streamBaseState, matching onStreamRead in stream_base_commons.ts.
    nativeHandle.onread = function (buf: ArrayBuffer | Uint8Array | undefined) {
      const nread = streamBaseState[kReadBytesOrError];
      if (nread > 0 && buf) {
        // LibUvStreamWrap passes an ArrayBuffer; convert to Uint8Array for receive()
        const data = buf instanceof ArrayBuffer
          ? new Uint8Array(buf, 0, nread)
          : buf.subarray(0, nread);
        res.receive(data);
      } else if (nread < 0) {
        // EOF or error - stop native TCP reads and unref the handle.
        // Without this, the libuv handle keeps a ref on the event loop
        // and prevents process exit after the TLS connection ends.
        nativeHandle.readStop();
        nativeHandle.unref();
        res.emitEof();
      }
    };

    // Store the native handle so readStart/readStop can delegate to it.
    res._nativeTcpHandle = nativeHandle;
  }

  // Store the JS handle reference so Rust can call JS callbacks (onhandshakedone, etc.)
  res.setHandle(res);

  return res;
}

export default { TLSWrap, wrap };
