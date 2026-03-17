// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any

import { TLSWrap } from "ext:core/ops";

export { TLSWrap };

/**
 * Create a TLSWrap that intercepts an underlying stream handle.
 * Mirrors Node's `internalBinding('tls_wrap').wrap(handle, context, isServer)`.
 *
 * @param handle - The underlying stream handle (TCP CppGC object)
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
  // TODO: use a proper async_id from async_hooks
  const asyncId = 0;
  const res = new TLSWrap(kind, asyncId);

  // initClientTls/initServerTls take the SecureContext JS object directly
  // and build the rustls config from its { ca, cert, key } properties.
  // The actual TLS connection is deferred until start() so that
  // setALPNProtocols can modify the config first.
  const initResult = isServer
    ? res.initServerTls(context)
    : res.initClientTls(servername || "localhost", context);
  if (initResult !== 0) {
    const err = new Error("unsupported protocol");
    // rustls cannot negotiate TLSv1.0/TLSv1.1, so surface the closest
    // OpenSSL-style error instead of hanging the socket.
    (err as Error & { code?: string }).code = "ERR_SSL_UNSUPPORTED_PROTOCOL";
    throw err;
  }

  // Attach to the underlying stream (TCP handle)
  const attachResult = res.attach(handle._nativeHandle ?? handle);
  if (attachResult !== 0) {
    throw new Error(`TLS wrap attach failed: ${attachResult}`);
  }

  // Store the JS handle reference so Rust can call JS callbacks (onhandshakedone, etc.)
  res.setHandle(res);

  return res;
}

export default { TLSWrap, wrap };
