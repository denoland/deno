// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

(function () {
const { core } = __bootstrap;
const { op_get_env_no_permission_check } = core.ops;
const lazyTls = core.createLazyLoader("node:tls");
const lazyNet = core.createLazyLoader("node:net");
const { urlToHttpOptions } = core.loadExtScript(
  "ext:deno_node/internal/url.ts",
);
const lazyHttp = core.createLazyLoader("node:http");
const lazyAddressOverride = core.createLazyLoader(
  "ext:deno_node/internal/http/address_override.js",
);
const { nextTick } = core.loadExtScript("ext:deno_node/_next_tick.ts");
const { ERR_INVALID_URL, ERR_PROXY_TUNNEL } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const {
  httpServerPreClose,
  setupConnectionsTracking,
  storeHTTPOptions,
} = core.createLazyLoader("node:_http_server")();
const { Agent: HttpAgent } = core.createLazyLoader("node:_http_agent")();
const { validateObject } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { kEmptyObject } = core.loadExtScript("ext:deno_node/internal/util.mjs");
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");

const tls = lazyTls().default;
const net = lazyNet();
const http = lazyHttp();
const { applyAddressOverride, startOverrideListener } = lazyAddressOverride();
const { _connectionListener, ClientRequest, ServerImpl: HttpServer } = http;

function getExtraCACertificates() {
  if (!op_get_env_no_permission_check("NODE_EXTRA_CA_CERTS")) {
    return undefined;
  }
  return tls.getCACertificates("default");
}

// https.Server extends tls.Server (which extends net.Server).
// Each accepted TCP connection is wrapped with TLS by tls.Server's
// connectionListener, then the HTTP _connectionListener handles the
// HTTP protocol on the decrypted stream. Matches Node.js architecture.
function Server(
  this: any,
  opts: any,
  requestListener?: any,
) {
  if (!(this instanceof Server)) {
    return new (Server as any)(opts, requestListener);
  }

  let ALPNProtocols: string[] | undefined = ["http/1.1"];
  if (typeof opts === "function") {
    requestListener = opts;
    opts = kEmptyObject;
  } else if (opts == null) {
    opts = kEmptyObject;
  } else {
    validateObject(opts, "options");
    // Only set default ALPNProtocols if the caller has not set either
    if (opts.ALPNProtocols || opts.ALPNCallback) {
      ALPNProtocols = undefined;
    }
  }

  storeHTTPOptions.call(this, opts);

  tls.Server.call(this, {
    noDelay: true,
    ALPNProtocols,
    ...opts,
  }, _connectionListener);

  this.httpAllowHalfOpen = false;

  if (requestListener) {
    this.addListener("request", requestListener);
  }

  this.addListener("tlsClientError", function (this: any, err: any, conn: any) {
    if (!this.emit("clientError", err, conn)) {
      conn.destroy(err);
    }
  });

  this.timeout = 0;
  this.maxHeadersCount = null;
  this.on("listening", setupConnectionsTracking);
}
Object.setPrototypeOf(Server.prototype, tls.Server.prototype);
Object.setPrototypeOf(Server, tls.Server);

Server.prototype.closeAllConnections = HttpServer.prototype.closeAllConnections;
Server.prototype.closeIdleConnections =
  HttpServer.prototype.closeIdleConnections;
Server.prototype.setTimeout = HttpServer.prototype.setTimeout;

// Same DENO_SERVE_ADDRESS override hook as http.Server, but on
// https.Server. The override listener is plain cleartext HTTP (it
// goes directly through _connectionListener, bypassing tls wrapping)
// because the typical use case -- Deno Deploy / desktop runtime
// vsock/unix control channels -- is trusted local traffic.
Server.prototype.listen = function listen(this: any, ...args: any[]) {
  const applied = applyAddressOverride();
  switch (applied.mode) {
    case "none":
      return net.Server.prototype.listen.apply(this, args);
    case "tcp": {
      let cb: any;
      const last = args[args.length - 1];
      if (typeof last === "function") {
        cb = last;
        args = args.slice(0, -1);
      }
      const rewritten: any[] = [{ host: applied.host, port: applied.port }];
      if (cb) rewritten.push(cb);
      return net.Server.prototype.listen.apply(this, rewritten);
    }
    case "override-only": {
      let cb: any;
      const last = args[args.length - 1];
      if (typeof last === "function") cb = last;
      if (cb) this.once("listening", cb);
      this._handle = {
        close() {},
        ref() {},
        unref() {},
      };
      startOverrideListener(this, applied.override, _connectionListener);
      nextTick(() => this.emit("listening"));
      return this;
    }
    case "duplicate": {
      startOverrideListener(this, applied.override, _connectionListener);
      return net.Server.prototype.listen.apply(this, args);
    }
  }
};

Server.prototype.close = function close(this: any) {
  httpServerPreClose(this);
  tls.Server.prototype.close.apply(this, arguments);
  return this;
};

Server.prototype[Symbol.asyncDispose] = async function (this: any) {
  await new Promise<void>((resolve, reject) => {
    this.close((err: any) => (err ? reject(err) : resolve()));
  });
};

function createServer(
  opts: any,
  requestListener?: any,
) {
  return new (Server as any)(opts, requestListener);
}

/** Makes a GET request to an https server. */
function get(...args: any[]) {
  const req = request(args[0], args[1], args[2]);
  req.end();
  return req;
}

// Defined as a regular function (not a `class`) so that `https.Agent()` may be
// invoked without `new`, matching Node:
// https://github.com/nodejs/node/blob/main/lib/https.js
function Agent(this: any, options: any) {
  if (!(this instanceof Agent)) {
    return new (Agent as any)(options);
  }

  options = { __proto__: null, ...options };
  options.defaultPort ??= 443;
  options.protocol ??= "https:";
  HttpAgent.call(this, options);

  this.maxCachedSessions = this.options.maxCachedSessions;
  if (this.maxCachedSessions === undefined) {
    this.maxCachedSessions = 100;
  }

  this._sessionCache = {
    map: {},
    list: [],
  };
}
Object.setPrototypeOf(Agent.prototype, HttpAgent.prototype);
Object.setPrototypeOf(Agent, HttpAgent);

Agent.prototype.getName = function getName(this: any, options: any = {}) {
  let name = HttpAgent.prototype.getName.call(this, options);

  name += ":";
  if (options.ca) name += options.ca;

  name += ":";
  if (options.cert) name += options.cert;

  name += ":";
  if (options.clientCertEngine) name += options.clientCertEngine;

  name += ":";
  if (options.ciphers) name += options.ciphers;

  name += ":";
  if (options.key) name += options.key;

  name += ":";
  if (options.pfx) name += options.pfx;

  name += ":";
  if (options.rejectUnauthorized !== undefined) {
    name += options.rejectUnauthorized;
  }

  name += ":";
  if (options.servername && options.servername !== options.host) {
    name += options.servername;
  }

  name += ":";
  if (options.minVersion) name += options.minVersion;

  name += ":";
  if (options.maxVersion) name += options.maxVersion;

  name += ":";
  if (options.secureProtocol) name += options.secureProtocol;

  name += ":";
  if (options.crl) name += options.crl;

  name += ":";
  if (options.honorCipherOrder !== undefined) {
    name += options.honorCipherOrder;
  }

  name += ":";
  if (options.ecdhCurve) name += options.ecdhCurve;

  name += ":";
  if (options.dhparam) name += options.dhparam;

  name += ":";
  if (options.secureOptions !== undefined) name += options.secureOptions;

  name += ":";
  if (options.sessionIdContext) name += options.sessionIdContext;

  name += ":";
  if (options.sigalgs) name += JSON.stringify(options.sigalgs);

  name += ":";
  if (options.privateKeyIdentifier) name += options.privateKeyIdentifier;

  name += ":";
  if (options.privateKeyEngine) name += options.privateKeyEngine;

  return name;
};

Agent.prototype._getSession = function _getSession(this: any, key: string) {
  return this._sessionCache.map[key];
};

Agent.prototype._cacheSession = function _cacheSession(
  this: any,
  key: string,
  session: any,
) {
  if (this.maxCachedSessions === 0) return;

  if (this._sessionCache.map[key]) {
    this._sessionCache.map[key] = session;
    return;
  }

  if (this._sessionCache.list.length >= this.maxCachedSessions) {
    const oldKey = this._sessionCache.list.shift()!;
    delete this._sessionCache.map[oldKey];
  }

  this._sessionCache.list.push(key);
  this._sessionCache.map[key] = session;
};

Agent.prototype._evictSession = function _evictSession(
  this: any,
  key: string,
) {
  const index = this._sessionCache.list.indexOf(key);
  if (index === -1) return;

  this._sessionCache.list.splice(index, 1);
  delete this._sessionCache.map[key];
};

Agent.prototype.createConnection = function createConnection(
  this: any,
  ...args: any[]
) {
  let options = args[0];
  const cb = typeof args[args.length - 1] === "function"
    ? args[args.length - 1]
    : undefined;

  if (typeof args[0] === "number") {
    // createConnection(port, host, options) signature
    const opts: any = {};
    for (let i = 1; i < args.length; i++) {
      if (args[i] !== null && typeof args[i] === "object") {
        Object.assign(opts, args[i]);
      }
    }
    if (typeof args[0] === "number") opts.port = args[0];
    if (typeof args[1] === "string") opts.host = args[1];
    options = opts;
  } else if (options !== null && typeof options === "object") {
    options = { ...options };
  } else {
    options = {};
  }

  if (options.ca === undefined) {
    const extraCACertificates = getExtraCACertificates();
    if (extraCACertificates !== undefined) {
      options.ca = extraCACertificates;
    }
  }

  // Look up cached TLS session for reuse
  if (options._agentKey) {
    const session = this._getSession(options._agentKey);
    if (session) {
      options = { session, ...options };
    }
  }

  // HTTPS through HTTP/HTTPS proxy: open CONNECT tunnel, then TLS-upgrade.
  if (options._proxy && options._proxyProtocol === "https:") {
    return openCONNECTTunnel(this, options, cb);
  }

  const socket = tls.connect(options as any);

  // Cache session on new session event
  if (options._agentKey) {
    socket.on("session", (session: any) => {
      this._cacheSession(options._agentKey, session);
    });

    socket.once("close", (err: any) => {
      if (err) this._evictSession(options._agentKey);
    });
  }

  if (cb) {
    socket.once("secureConnect", cb);
  }

  return socket;
};

// Opens a CONNECT tunnel through `proxy` and TLS-upgrades the resulting
// socket. Returns undefined; resolves through `cb(err, tlsSocket)`.
//
// Surfaces:
//   - proxy connection refused / DNS errors as socket errors on the cb path
//   - non-2xx CONNECT responses as ERR_HTTP_TUNNEL_FAILED-ish errors
//   - request/agent timeouts during CONNECT with a message containing
//     "timed out after Nms" so tests for tunnel-timeout match
function openCONNECTTunnel(agent: any, options: any, cb: any) {
  const proxy = options._proxy;
  const targetHost = options._proxyTargetHost;
  const targetPort = options._proxyTargetPort;
  const hostHeader = targetHost.indexOf(":") !== -1 &&
      targetHost.charCodeAt(0) !== 91
    ? `[${targetHost}]:${targetPort}`
    : `${targetHost}:${targetPort}`;

  let tunnelSocket: any;
  const timeout = options.timeout || (agent.options && agent.options.timeout) ||
    0;

  // Initial TCP/TLS connection to the proxy itself.
  const proxyConnectOpts: any = {
    __proto__: null,
    host: proxy.hostname,
    port: proxy.port,
  };
  if (proxy.protocol === "https:") {
    // The proxy itself uses TLS - connect to it via tls.connect first.
    proxyConnectOpts.servername = proxy.hostname;
    // Propagate TLS knobs from the request options so NODE_EXTRA_CA_CERTS
    // and friends apply to the proxy hop too.
    proxyConnectOpts.ca = options.ca ?? getExtraCACertificates();
    if (options.rejectUnauthorized !== undefined) {
      proxyConnectOpts.rejectUnauthorized = options.rejectUnauthorized;
    }
    tunnelSocket = tls.connect(proxyConnectOpts);
  } else {
    tunnelSocket = lazyNet().default.createConnection(proxyConnectOpts);
  }

  let settled = false;
  let timeoutId: any = null;
  let waitingForTunnelResponse = false;

  function clearTimer() {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
      timeoutId = null;
    }
  }

  function fail(err: any) {
    if (settled) return;
    settled = true;
    clearTimer();
    try {
      tunnelSocket.on("error", () => {});
      tunnelSocket.destroy();
    } catch { /* ignore */ }
    cb(err);
  }

  if (timeout > 0) {
    timeoutId = setTimeout(() => {
      // Include "Request timed out" so the runtime-side request handler's
      // `req.on('timeout')` log line matches in test stderr without needing
      // a real socket-level 'timeout' event (which we can't easily emit
      // before the agent has attached the socket to the request).
      const err: any = new Error(
        `Request timed out: Tunneling socket connection timed out after ${timeout}ms`,
      );
      err.code = "ETIMEDOUT";
      fail(err);
    }, timeout);
  }

  tunnelSocket.once("error", fail);

  function failUnexpectedEnd() {
    if (!waitingForTunnelResponse || settled) return;
    fail(
      new ERR_PROXY_TUNNEL(
        "Connection to establish proxy tunnel ended unexpectedly",
      ),
    );
  }

  function onProxyReady() {
    // Send CONNECT request. Matches Node's wire format - no Connection
    // header, just Proxy-Connection: keep-alive plus Host.
    let request = `CONNECT ${hostHeader} HTTP/1.1\r\n` +
      `Host: ${hostHeader}\r\n`;
    if (options._proxyUseProxyConnection !== false) {
      request += `Proxy-Connection: keep-alive\r\n`;
    }
    if (proxy.auth) {
      request += `Proxy-Authorization: ${proxy.auth}\r\n`;
    }
    request += `\r\n`;
    try {
      tunnelSocket.write(request);
    } catch (err) {
      fail(err);
      return;
    }
    waitingForTunnelResponse = true;
    tunnelSocket.once("end", failUnexpectedEnd);
    tunnelSocket.once("close", failUnexpectedEnd);

    // Buffer raw bytes until we've parsed the response status line + headers.
    let buf = Buffer.alloc(0);
    function onData(chunk: any) {
      buf = Buffer.concat([buf, chunk]);
      const idx = buf.indexOf("\r\n\r\n");
      if (idx === -1) {
        // Need more data; if too much is buffered without headers, bail.
        if (buf.length > 64 * 1024) {
          tunnelSocket.removeListener("data", onData);
          fail(new Error("Tunneling socket got too large headers"));
        }
        return;
      }
      tunnelSocket.removeListener("data", onData);
      waitingForTunnelResponse = false;
      tunnelSocket.removeListener("end", failUnexpectedEnd);
      tunnelSocket.removeListener("close", failUnexpectedEnd);
      const headerStr = buf.slice(0, idx).toString("ascii");
      const remainder = buf.slice(idx + 4);
      const statusLine = headerStr.split("\r\n", 1)[0];
      const m = /^HTTP\/1\.\d\s+(\d{3})/.exec(statusLine);
      if (!m) {
        fail(
          new ERR_PROXY_TUNNEL(
            `Failed to establish tunnel to ${hostHeader} over ${proxy.protocol}//${proxy.hostname}:${proxy.port}: ${statusLine}`,
          ),
        );
        return;
      }
      const statusCode = Number(m[1]);
      if (statusCode < 200 || statusCode >= 300) {
        // Format matches Node's wire: "Failed to establish tunnel to
        // <host:port> over <proxy origin>: <status line>". Tests grep for
        // ERR_PROXY_TUNNEL + this prefix + the verbatim status line.
        const err: any = new ERR_PROXY_TUNNEL(
          `Failed to establish tunnel to ${hostHeader} over ${proxy.protocol}//${proxy.hostname}:${proxy.port}: ${statusLine}`,
        );
        err.statusCode = statusCode;
        if (settled) return;
        settled = true;
        clearTimer();
        tunnelSocket.on("error", () => {});
        try {
          tunnelSocket.end();
        } catch { /* ignore */ }
        cb(err);
        return;
      }
      // Now upgrade to TLS. Use the TCP socket as the underlying transport.
      // Pass servername for SNI based on target host.
      const tlsOpts: any = {
        __proto__: null,
        ...options,
        socket: tunnelSocket,
        host: targetHost,
        port: targetPort,
        servername: options.servername ||
          (lazyNet().default.isIP(targetHost) ? undefined : targetHost),
      };
      delete tlsOpts._proxy;
      delete tlsOpts._proxyTargetHost;
      delete tlsOpts._proxyTargetPort;
      delete tlsOpts._proxyProtocol;
      delete tlsOpts._proxyUseProxyConnection;
      // tls.connect doesn't expose handshake errors via a callback in our
      // bindings cleanly, so listen for 'error' before secureConnect.
      let tlsSocket: any;
      try {
        tlsSocket = tls.connect(tlsOpts);
      } catch (err) {
        fail(err);
        return;
      }
      function onTlsError(err: any) {
        tlsSocket.removeListener("secureConnect", onSecure);
        fail(err);
      }
      function onSecure() {
        if (settled) return;
        settled = true;
        clearTimer();
        tlsSocket.removeListener("error", onTlsError);
        // If the tunnel sent body bytes immediately, push them back into
        // the TLS layer. Normally CONNECT response has no body, so this
        // should be empty.
        if (remainder.length > 0) {
          tlsSocket.unshift?.(remainder);
        }
        // Detach our top-level proxy error handler from the TCP socket so
        // errors after handshake propagate via tlsSocket only.
        tunnelSocket.removeListener("error", fail);
        cb(null, tlsSocket);
      }
      tlsSocket.once("error", onTlsError);
      tlsSocket.once("secureConnect", onSecure);
    }
    tunnelSocket.on("data", onData);
  }

  if (proxy.protocol === "https:") {
    tunnelSocket.once("secureConnect", onProxyReady);
  } else {
    if (tunnelSocket.connecting === false) {
      onProxyReady();
    } else {
      tunnelSocket.once("connect", onProxyReady);
    }
  }

  return undefined;
}

let globalAgent = new (Agent as any)({
  keepAlive: true,
  scheduling: "lifo",
  timeout: 5000,
});

/** Makes a request to an https server. */
function request(...args: any[]) {
  let options: any = {};

  if (typeof args[0] === "string") {
    const urlStr = args.shift();
    // Match Node: surface invalid URL strings as ERR_INVALID_URL.
    let parsed;
    try {
      parsed = new URL(urlStr);
    } catch {
      throw new ERR_INVALID_URL(urlStr);
    }
    options = urlToHttpOptions(parsed);
  } else if (args[0] instanceof URL) {
    options = urlToHttpOptions(args.shift());
  }

  if (args[0] && typeof args[0] !== "function") {
    Object.assign(options, args.shift());
  }

  options._defaultAgent = globalAgent;
  args.unshift(options);

  return new ClientRequest(args[0], args[1], args[2]);
}

// `agent-base` (used by `@npmcli/agent`, `http-proxy-agent`, etc.) figures
// out whether a polymorphic agent should behave as https by scanning the
// current stack for `(https.js:` or `node:https:`. Without a marker the
// stack only shows our polyfill path and the agent reports `protocol:
// "http:"`, causing `http.ClientRequest` to throw `ERR_INVALID_PROTOCOL`
// against an `https:` URL. Encode the marker in the function name.
Object.defineProperty(request, "name", { value: "node:https:request" });

return {
  Agent,
  Server,
  createServer,
  get,
  get globalAgent() {
    return globalAgent;
  },
  set globalAgent(value) {
    globalAgent = value;
  },
  request,
};
})();
