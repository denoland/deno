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

const {
  Error,
  FunctionPrototypeCall,
  Number,
  Promise,
  PromisePrototypeThen,
  Symbol,
  SymbolAsyncIterator,
  Uint8Array,
} = primordials;

const { nextTick } = core.loadExtScript("ext:deno_node/_next_tick.ts");
const { listen: denoListen } = core.loadExtScript("ext:deno_net/01_net.js");

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

function notifyAddressOverrideServing() {
  op_http_notify_serving();
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
  #readBuf = new Uint8Array(64 * 1024);
  #timeoutMsecs = 0;
  #timeoutTimer = null;
  // Node's HTTP server uses these directly.
  isDenoServeAddressOverride = true;
  _paused = false;
  server = null;
  parser = null;
  _httpMessage = null;
  encrypted = false;
  remoteAddress = null;
  remotePort = null;
  remoteFamily = null;

  constructor(conn) {
    super({ allowHalfOpen: true });
    this.#conn = conn;

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
      while (!this.#closed) {
        let n;
        try {
          n = await this.#conn.read(this.#readBuf);
        } catch (err) {
          this.destroy(err);
          return;
        }
        if (n === null) {
          // deno-lint-ignore prefer-primordials
          this.push(null);
          return;
        }
        // Any byte resets the idle timer.
        this.#armTimer();
        // Copy into a Buffer so the caller owns it independently of
        // our reusable read buffer.
        const chunk = Buffer.from(this.#readBuf.subarray(0, n));
        // deno-lint-ignore prefer-primordials
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

  _write(chunk, encoding, callback) {
    const bytes = typeof chunk === "string"
      ? Buffer.from(chunk, encoding)
      : chunk;
    // Any outgoing byte resets the idle timer too, matching net.Socket.
    this.#armTimer();
    PromisePrototypeThen(this.#conn.write(bytes), () => callback(), callback);
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

// Run a Deno listener on the override address, handing each accepted
// connection to `connectionListener` so the HTTP parser picks it up
// the same way a regular net.Server connection would. For https.Server
// the caller should pass the plain http _connectionListener so the
// override channel stays cleartext (typical Deno Deploy / desktop
// runtime use case: trusted local vsock/unix transport).
function startOverrideListener(server, override, connectionListener) {
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
  notifyAddressOverrideServing();

  (async () => {
    try {
      const it = denoListener[SymbolAsyncIterator]();
      while (true) {
        // deno-lint-ignore prefer-primordials
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
        const socket = new OverrideSocket(conn);
        FunctionPrototypeCall(connectionListener, server, socket);
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
  startOverrideListener,
};
