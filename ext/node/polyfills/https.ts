// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

import { core } from "ext:core/mod.js";
import tls from "node:tls";
import { urlToHttpOptions } from "ext:deno_node/internal/url.ts";
import {
  _connectionListener,
  ClientRequest,
  type ServerHandler,
  ServerImpl as HttpServer,
} from "node:http";
const { ERR_INVALID_URL } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
import {
  httpServerPreClose,
  setupConnectionsTracking,
  storeHTTPOptions,
} from "node:_http_server";
import { Agent as HttpAgent } from "node:_http_agent";
const { validateObject } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { kEmptyObject } = core.loadExtScript("ext:deno_node/internal/util.mjs");

// https.Server extends tls.Server (which extends net.Server).
// Each accepted TCP connection is wrapped with TLS by tls.Server's
// connectionListener, then the HTTP _connectionListener handles the
// HTTP protocol on the decrypted stream. Matches Node.js architecture.
export function Server(
  this: any,
  opts: any,
  requestListener?: ServerHandler,
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

export function createServer(
  opts: any,
  requestListener?: ServerHandler,
) {
  return new (Server as any)(opts, requestListener);
}

/** Makes a GET request to an https server. */
export function get(...args: any[]) {
  const req = request(args[0], args[1], args[2]);
  req.end();
  return req;
}

// Defined as a regular function (not a `class`) so that `https.Agent()` may be
// invoked without `new`, matching Node:
// https://github.com/nodejs/node/blob/main/lib/https.js
export function Agent(this: any, options: any) {
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
  options: any,
  cb?: any,
) {
  if (typeof options === "number") {
    // createConnection(port, host, options) signature
    const args = arguments;
    const opts: any = {};
    if (args[0] !== null && typeof args[0] === "object") {
      Object.assign(opts, args[0]);
    } else if (args[1] !== null && typeof args[1] === "object") {
      Object.assign(opts, args[1]);
    } else if (args[2] !== null && typeof args[2] === "object") {
      Object.assign(opts, args[2]);
    }
    if (typeof args[0] === "number") opts.port = args[0];
    if (typeof args[1] === "string") opts.host = args[1];
    if (typeof args[args.length - 1] === "function") {
      cb = args[args.length - 1];
    }
    options = opts;
  }

  // Look up cached TLS session for reuse
  if (options._agentKey) {
    const session = this._getSession(options._agentKey);
    if (session) {
      options = { session, ...options };
    }
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

export const globalAgent = new Agent({
  keepAlive: true,
  scheduling: "lifo",
  timeout: 5000,
});

/** Makes a request to an https server. */
export function request(...args: any[]) {
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

export default {
  Agent,
  Server,
  createServer,
  get,
  globalAgent,
  request,
};
