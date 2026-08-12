// Copyright 2018-2026 the Deno authors. MIT license.

// Support for DENO_SERVE_ADDRESS override in Node.js http.Server.
//
// When DENO_SERVE_ADDRESS is set, node:http servers behave like the
// top-level Deno.serve(): the first server to call `listen()` consumes
// the override.
//
// - Without the `duplicate` flag, the server listens on the override
//   address instead of the address the application requested.
// - With the `duplicate` flag, the server listens on BOTH addresses.
//
// TCP overrides are applied by mutating the listen() options so the
// normal net.Server path handles them. Non-TCP overrides (unix, vsock,
// tunnel) open a Deno listener directly and feed each accepted
// connection to the server's connection listener via a lightweight
// Duplex wrapper around the Deno.Conn.

import { core, primordials } from "ext:core/mod.js";
import {
  op_http_notify_serving,
  op_http_serve_address_override,
} from "ext:core/ops";

import { Buffer } from "node:buffer";
import { Duplex } from "node:stream";
import { clearTimeout, setTimeout } from "node:timers";

const {
  ArrayPrototypePush,
  Error,
  FunctionPrototypeCall,
  ObjectDefineProperty,
  Number,
  Promise,
  PromisePrototypeCatch,
  PromisePrototypeThen,
  SafePromiseRace,
  Symbol,
  SymbolAsyncIterator,
  TypedArrayPrototypeSubarray,
  Uint8Array,
} = primordials;

const { nextTick } = core.loadExtScript("ext:deno_node/_next_tick.ts");
const { listen: denoListen } = core.loadExtScript("ext:deno_net/01_net.js");
const { ERR_SOCKET_CLOSED } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);

// Has the process-global override been consumed by some server yet?
let addressOverrideConsumed = false;

// Override kinds returned by op_http_serve_address_override().
const KIND_NONE = 0;
const KIND_TCP = 1;
const KIND_UNIX = 2;
const KIND_VSOCK = 3;
const KIND_TUNNEL = 4;

// Peek at the override without consuming it. Returns null if there is
// no override or it has already been consumed by an earlier server.
function peekOverride() {
  if (addressOverrideConsumed) return null;
  const result = op_http_serve_address_override();
  const kind = result[0];
  if (kind === KIND_NONE) return null;
  return { kind, host: result[1], port: result[2], duplicate: result[3] };
}

// Mark the override as consumed. Subsequent servers see no override.
function consumeOverride() {
  addressOverrideConsumed = true;
}

// Server kind codes for op_http_notify_serving. Must match
// `serving_server_kind()` in ext/http/lib.rs.
const SERVER_KIND_NODE_HTTP = 2;
const SERVER_KIND_NODE_HTTP2 = 3;

function notifyAddressOverrideServing(kind = SERVER_KIND_NODE_HTTP) {
  op_http_notify_serving(kind);
}

// Translate an override record into the argument for denoListen().
function overrideToListenArgs(override) {
  switch (override.kind) {
    case KIND_TCP:
      return { hostname: override.host, port: override.port };
    case KIND_UNIX:
      return { transport: "unix", path: override.host };
    case KIND_VSOCK:
      return {
        transport: "vsock",
        cid: Number(override.host),
        port: override.port,
      };
    case KIND_TUNNEL:
      return { transport: "tunnel" };
    default:
      throw new Error(`unknown override kind: ${override.kind}`);
  }
}

// Minimal Socket-like Duplex wrapping a Deno.Conn. It exposes the
// subset of the net.Socket interface that connectionListener in
// _http_server.js actually uses.
class OverrideSocket extends Duplex {
  #conn;
  #closed = false;
  #initialChunk = null;
  #readBuf = new Uint8Array(64 * 1024);
  #timeoutMsecs = 0;
  #timeoutTimer = null;
  // Node's HTTP server uses these directly.
  isDenoServeAddressOverride = true;
  // Set by the alpn-routing dispatch (node:http2 servers) to the sniffed
  // protocol, mirroring what a TLS socket would report.
  alpnProtocol = undefined;
  _paused = false;
  server = null;
  parser = null;
  _httpMessage = null;
  encrypted = false;
  remoteAddress = null;
  remotePort = null;
  remoteFamily = null;

  constructor(conn, initialChunk) {
    super({ allowHalfOpen: true });
    this.#conn = conn;
    this.#initialChunk = initialChunk;

    const remote = conn.remoteAddr;
    if (remote && remote.transport === "tcp") {
      this.remoteAddress = remote.hostname;
      this.remotePort = remote.port;
      this.remoteFamily = remote.hostname?.includes(":") ? "IPv6" : "IPv4";
    } else if (remote && remote.transport === "unix") {
      this.remoteAddress = remote.path ?? "";
    } else if (remote && remote.transport === "vsock") {
      this.remoteAddress = `vsock:${remote.cid}`;
      this.remotePort = remote.port;
    }

    this.#readLoop();
  }

  #armTimer() {
    if (this.#timeoutTimer !== null) clearTimeout(this.#timeoutTimer);
    if (this.#timeoutMsecs > 0 && !this.#closed) {
      this.#timeoutTimer = setTimeout(() => {
        this.#timeoutTimer = null;
        if (!this.#closed) this.emit("timeout");
      }, this.#timeoutMsecs);
    }
  }

  async #readLoop() {
    try {
      if (this.#initialChunk !== null && this.#initialChunk.length > 0) {
        const chunk = Buffer.from(this.#initialChunk);
        this.#initialChunk = null;
        // deno-lint-ignore deno-internal/prefer-primordials
        if (!this.push(chunk)) {
          await new Promise((resolve) => {
            this._readResume = resolve;
          });
          this._readResume = null;
        }
      } else {
        this.#initialChunk = null;
      }
      while (!this.#closed) {
        let n;
        try {
          n = await this.#conn.read(this.#readBuf);
        } catch (err) {
          this.destroy(err);
          return;
        }
        if (n === null) {
          // deno-lint-ignore deno-internal/prefer-primordials
          this.push(null);
          return;
        }
        // Any byte resets the idle timer.
        this.#armTimer();
        // Copy into a Buffer so the caller owns it independently of
        // our reusable read buffer.
        const chunk = Buffer.from(this.#readBuf.subarray(0, n));
        // deno-lint-ignore deno-internal/prefer-primordials
        if (!this.push(chunk)) {
          // Backpressure: wait until _read() is called before reading
          // more from the connection.
          await new Promise((resolve) => {
            this._readResume = resolve;
          });
          this._readResume = null;
        }
      }
    } catch (err) {
      this.destroy(err);
    }
  }

  _read(_size) {
    if (this._readResume) {
      const resume = this._readResume;
      this._readResume = null;
      resume();
    }
  }

  // Deno.Conn.write() is a single syscall-like write: it may write fewer
  // bytes than provided (e.g. when the transport send buffer is full --
  // vsock buffers are 64 KiB). Loop until the whole chunk is flushed.
  async #writeAll(bytes) {
    let nwritten = await this.#conn.write(bytes);
    while (nwritten < bytes.length) {
      const n = await this.#conn.write(
        TypedArrayPrototypeSubarray(bytes, nwritten, bytes.length),
      );
      if (n === 0) {
        throw new ERR_SOCKET_CLOSED();
      }
      nwritten += n;
    }
  }

  _write(chunk, encoding, callback) {
    const bytes = typeof chunk === "string"
      ? Buffer.from(chunk, encoding)
      : chunk;
    // Any outgoing byte resets the idle timer too, matching net.Socket.
    this.#armTimer();
    PromisePrototypeThen(this.#writeAll(bytes), () => callback(), callback);
  }

  _final(callback) {
    try {
      // Allow the other side to finish reading while we finish writing.
      if (this.#conn.closeWrite) {
        PromisePrototypeThen(
          this.#conn.closeWrite(),
          () => callback(),
          callback,
        );
        return;
      }
    } catch (_) {
      // fall through to close
    }
    callback();
  }

  _destroy(err, callback) {
    if (!this.#closed) {
      this.#closed = true;
      try {
        this.#conn.close();
      } catch (_) {
        // ignore
      }
    }
    if (this.#timeoutTimer !== null) {
      clearTimeout(this.#timeoutTimer);
      this.#timeoutTimer = null;
    }
    if (this._readResume) {
      const resume = this._readResume;
      this._readResume = null;
      resume();
    }
    callback(err);
  }

  // Matches net.Socket#setTimeout: 0 disables, non-zero arms an idle
  // timer that fires "timeout" when no I/O happens for that long.
  // The HTTP server uses this for keep-alive / headers / request
  // timeouts.
  setTimeout(msecs, cb) {
    this.#timeoutMsecs = msecs | 0;
    if (typeof cb === "function") this.on("timeout", cb);
    this.#armTimer();
    return this;
  }
  setNoDelay() {
    return this;
  }
  setKeepAlive() {
    return this;
  }

  address() {
    const local = this.#conn.localAddr;
    if (!local) return null;
    if (local.transport === "tcp") {
      return { address: local.hostname, port: local.port, family: "IPv4" };
    }
    return null;
  }

  ref() {
    return this;
  }
  unref() {
    return this;
  }
}

// The HTTP/2 client connection preface: "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".
// Control planes (e.g. Deno Deploy's deployd) forward HTTP/2 requests to the
// override listener with prior knowledge, so node:http servers must accept
// both HTTP/1.1 and HTTP/2 connections here, like Deno.serve() does.
// deno-fmt-ignore
const H2_PREFACE = [
  0x50, 0x52, 0x49, 0x20, 0x2a, 0x20, 0x48, 0x54, 0x54, 0x50, 0x2f, 0x32,
  0x2e, 0x30, 0x0d, 0x0a, 0x0d, 0x0a, 0x53, 0x4d, 0x0d, 0x0a, 0x0d, 0x0a,
];

// How long a freshly accepted connection may take to send enough bytes to
// identify its protocol before it is dropped. The node server's own header
// timeouts only start once the connection is handed to it, so this guards
// the sniffing window against connections that never send anything.
const SNIFF_TIMEOUT_MS = 60_000;

// Reads just enough of the connection to decide whether the client is
// speaking HTTP/2 (prior knowledge) or HTTP/1.x. Never reads past the
// 24-byte HTTP/2 preface. Returns the consumed bytes so the HTTP/1.x
// path can replay them into the parser. Throws on timeout.
async function sniffConnection(conn) {
  const buf = new Uint8Array(H2_PREFACE.length);
  let filled = 0;
  let timer;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(
      () => reject(new Error("protocol sniffing timed out")),
      SNIFF_TIMEOUT_MS,
    );
  });
  try {
    while (filled < H2_PREFACE.length) {
      let n;
      try {
        n = await SafePromiseRace([
          conn.read(TypedArrayPrototypeSubarray(buf, filled)),
          timeout,
        ]);
      } catch (err) {
        if (err?.message === "protocol sniffing timed out") throw err;
        return {
          isH2: false,
          initial: TypedArrayPrototypeSubarray(buf, 0, filled),
        };
      }
      if (n === null) {
        return {
          isH2: false,
          initial: TypedArrayPrototypeSubarray(buf, 0, filled),
        };
      }
      for (let i = filled; i < filled + n; i++) {
        if (buf[i] !== H2_PREFACE[i]) {
          return {
            isH2: false,
            initial: TypedArrayPrototypeSubarray(buf, 0, filled + n),
          };
        }
      }
      filled += n;
    }
    return { isH2: true, initial: buf };
  } finally {
    clearTimeout(timer);
  }
}

// HTTP/2 connections accepted on the override listener are served by a
// real node:http2 server session over the same socket wrapper: HTTP/2
// stays HTTP/2 (native trailers, flow control), and requests reach the
// node server's "request" listeners as Http2ServerRequest /
// Http2ServerResponse compat objects -- the same objects node delivers
// for `http2.createSecureServer({ allowHTTP1: true })` servers.
//
// node:http2 is loaded lazily to avoid a module cycle (http2 depends on
// node:http internals).
let lazyHttp2;
function loadHttp2() {
  lazyHttp2 ??= core.loadExtScript("ext:deno_node/http2.ts");
  return lazyHttp2;
}

const kH2Shadow = Symbol("http.overrideH2Shadow");

// Returns a non-listening Http2Server bound to `server` that forwards
// requests (and the events frameworks commonly use) to it.
function h2ShadowServer(server) {
  let shadow = server[kH2Shadow];
  if (shadow !== undefined) return shadow;
  const http2 = loadHttp2();
  shadow = http2.createServer({});
  server[kH2Shadow] = shadow;
  shadow.on("request", (req, res) => {
    // The target server's listeners were written for node:http requests:
    // hide the HTTP/2 pseudo-headers (frameworks commonly copy req.headers
    // into a web Headers object, which rejects ":"-prefixed names) and
    // synthesize the host header an HTTP/1.1 request would carry. Hiding
    // them from the public `req.headers` view below is safe because
    // Http2ServerRequest.method / .url read `:method` / `:path` straight
    // from the internal kHeaders object (see internal/http2/compat.js), not
    // through the `headers` getter we override -- so the internal object is
    // left untouched and those accessors keep working.
    const headers = req.headers;
    // Note: a plain Object.prototype-backed object and settable
    // properties, matching the http1 IncomingMessage the listener was
    // written against (frameworks reassign req.headers and call
    // req.headers.hasOwnProperty()).
    let headersView = {};
    for (const name in headers) {
      if (name[0] === ":") continue;
      if (name === "__proto__") {
        // `headersView[name] = ...` would hit Object.prototype's `__proto__`
        // setter and silently drop the header, leaving it present in
        // rawHeaders but missing from headers. Define a real own property so
        // the two views stay consistent.
        ObjectDefineProperty(headersView, name, {
          __proto__: null,
          value: headers[name],
          writable: true,
          enumerable: true,
          configurable: true,
        });
      } else {
        headersView[name] = headers[name];
      }
    }
    if (
      headersView.host === undefined && headers[":authority"] !== undefined
    ) {
      headersView.host = headers[":authority"];
    }
    ObjectDefineProperty(req, "headers", {
      __proto__: null,
      configurable: true,
      enumerable: true,
      get: () => headersView,
      set: (value) => {
        headersView = value;
      },
    });
    const rawHeaders = req.rawHeaders;
    let hasHost = false;
    let rawView = [];
    for (let i = 0; i < rawHeaders.length; i += 2) {
      const name = rawHeaders[i];
      if (name[0] === ":") continue;
      if (name === "host") hasHost = true;
      ArrayPrototypePush(rawView, name, rawHeaders[i + 1]);
    }
    if (!hasHost && headersView.host !== undefined) {
      ArrayPrototypePush(rawView, "host", headersView.host);
    }
    ObjectDefineProperty(req, "rawHeaders", {
      __proto__: null,
      configurable: true,
      enumerable: true,
      get: () => rawView,
      set: (value) => {
        rawView = value;
      },
    });
    server.emit("request", req, res);
  });
  // Surface session/stream errors on the http.Server like client
  // connection errors; never let them become uncaught "error" events on
  // the shadow.
  shadow.on("sessionError", (err, session) => {
    const socket = session?.socket;
    // Only surface as clientError when there is a socket for the
    // listener to act on; otherwise (and when unhandled) tear the
    // session down directly.
    if (!socket || !server.emit("clientError", err, socket)) {
      session?.destroy(err);
    }
  });
  server.once("close", () => {
    shadow.close();
  });
  return shadow;
}

function startH2OverrideConnection(server, conn, initialChunk) {
  const socket = new OverrideSocket(conn, initialChunk);
  const shadow = h2ShadowServer(server);
  // net.Server registers its connection listener for the "connection"
  // event; emitting it runs the full HTTP/2 session setup on `socket`.
  shadow.emit("connection", socket);
}

// Run a Deno listener on the override address, handing each accepted
// connection to `connectionListener` so the HTTP parser picks it up
// the same way a regular net.Server connection would. For https.Server
// the caller should pass the plain http _connectionListener so the
// override channel stays cleartext (typical Deno Deploy / desktop
// runtime use case: trusted local vsock/unix transport).
// `alpnRouting` is used by node:http2 servers: instead of the
// http.Server-specific dispatch (h1 parser path / shadow HTTP/2 session),
// the sniffed protocol is exposed as `socket.alpnProtocol` ("h2" or
// "http/1.1") and every connection is handed to `connectionListener`,
// which performs its own protocol routing exactly like it would for a
// TLS+ALPN connection.
function startOverrideListener(
  server,
  override,
  connectionListener,
  alpnRouting = false,
) {
  let denoListener;
  try {
    denoListener = denoListen(overrideToListenArgs(override));
  } catch (err) {
    // Don't bring the main listener down just because the override
    // failed to open -- surface it asynchronously.
    nextTick(() => server.emit("error", err));
    return;
  }

  server[kOverrideListener] = denoListener;
  notifyAddressOverrideServing(
    alpnRouting ? SERVER_KIND_NODE_HTTP2 : SERVER_KIND_NODE_HTTP,
  );

  (async () => {
    try {
      const it = denoListener[SymbolAsyncIterator]();
      while (true) {
        // deno-lint-ignore deno-internal/prefer-primordials
        const { done, value: conn } = await it.next();
        if (done) break;
        if (server[kOverrideClosed]) {
          try {
            conn.close();
          } catch (_) {
            // ignore
          }
          break;
        }
        // Sniff the protocol off the accept loop so a slow client can't
        // stall other connections.
        PromisePrototypeCatch(
          (async () => {
            const { isH2, initial } = await sniffConnection(conn);
            if (server[kOverrideClosed]) {
              try {
                conn.close();
              } catch (_) {
                // ignore
              }
              return;
            }
            if (alpnRouting) {
              const socket = new OverrideSocket(conn, initial);
              socket.server = server;
              socket.alpnProtocol = isH2 ? "h2" : "http/1.1";
              FunctionPrototypeCall(connectionListener, server, socket);
            } else if (isH2) {
              startH2OverrideConnection(server, conn, initial);
            } else {
              const socket = new OverrideSocket(conn, initial);
              FunctionPrototypeCall(connectionListener, server, socket);
            }
          })(),
          (_err) => {
            try {
              conn.close();
            } catch (_) {
              // ignore
            }
          },
        );
      }
    } catch (err) {
      // Ignore BadResource on close.
      if (err && err.name !== "BadResource") {
        server.emit("error", err);
      }
    }
  })();

  // Shut the override listener down when the server closes.
  server.once("close", () => {
    server[kOverrideClosed] = true;
    try {
      denoListener.close();
    } catch (_) {
      // already closed
    }
  });
}

const kOverrideListener = Symbol("http.overrideListener");
const kOverrideClosed = Symbol("http.overrideClosed");

// Called from Server.prototype.listen to apply the override. Returns
// one of:
//   { mode: "none" }                          - no override in effect
//   { mode: "tcp", host, port }               - rewrite listen args to
//                                               the override TCP addr
//                                               (non-duplicate only)
//   { mode: "override-only", override }       - skip user listen, run
//                                               override listener only
//   { mode: "duplicate", override }           - run normal listen AND
//                                               override listener
function applyAddressOverride() {
  const override = peekOverride();
  if (!override) return { mode: "none" };

  consumeOverride();

  if (override.duplicate) {
    return { mode: "duplicate", override };
  }
  if (override.kind === KIND_TCP) {
    return { mode: "tcp", host: override.host, port: override.port };
  }
  return { mode: "override-only", override };
}

export {
  applyAddressOverride,
  notifyAddressOverrideServing,
  SERVER_KIND_NODE_HTTP,
  SERVER_KIND_NODE_HTTP2,
  startOverrideListener,
};
