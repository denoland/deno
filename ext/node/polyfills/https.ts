// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import tls from "node:tls";
import { urlToHttpOptions } from "ext:deno_node/internal/url.ts";
import {
  _connectionListener,
  ClientRequest,
  IncomingMessageForClient as IncomingMessage,
  type RequestOptions,
  ServerResponse,
} from "node:http";
import {
  httpServerPreClose,
  kServerResponse,
  setupConnectionsTracking,
  storeHTTPOptions,
} from "node:_http_server";
import { Agent as HttpAgent } from "node:_http_agent";
import { createHttpClient } from "ext:deno_fetch/22_http_client.js";
import { type ServerHandler, ServerImpl as HttpServer } from "node:http";
import { validateObject } from "ext:deno_node/internal/validators.mjs";
import { kEmptyObject } from "ext:deno_node/internal/util.mjs";

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

export function createServer(
  opts: any,
  requestListener?: ServerHandler,
) {
  return new (Server as any)(opts, requestListener);
}

interface HttpsRequestOptions extends RequestOptions {
  _: unknown;
}

// Store additional root CAs.
// undefined means NODE_EXTRA_CA_CERTS is not checked yet.
// null means there's no additional root CAs.
let caCerts: string[] | undefined | null;

/** Makes a request to an https server. */
export function get(
  url: string | URL,
  cb?: (res: IncomingMessage) => void,
): HttpsClientRequest;
export function get(
  opts: HttpsRequestOptions,
  cb?: (res: IncomingMessage) => void,
): HttpsClientRequest;
export function get(
  url: string | URL,
  opts: HttpsRequestOptions,
  cb?: (res: IncomingMessage) => void,
): HttpsClientRequest;
// deno-lint-ignore no-explicit-any
export function get(...args: any[]) {
  const req = request(args[0], args[1], args[2]);
  req.end();
  return req;
}

export class Agent extends HttpAgent {
  constructor(options) {
    super(options);
    this.defaultPort = 443;
    this.protocol = "https:";
    this.maxCachedSessions = this.options.maxCachedSessions;
    if (this.maxCachedSessions === undefined) {
      this.maxCachedSessions = 100;
    }

    this._sessionCache = {
      map: {},
      list: [],
    };
  }

  createConnection(options, callback) {
    // deno-lint-ignore no-explicit-any
    const socket = tls.connect(options as any);
    if (callback) {
      socket.once("secureConnect", callback);
    }
    return socket;
  }
}

export const globalAgent = new Agent({
  keepAlive: true,
  scheduling: "lifo",
  timeout: 5000,
});

/** HttpsClientRequest class loosely follows http.ClientRequest class API. */
class HttpsClientRequest extends ClientRequest {
  override _encrypted = true;
  override defaultProtocol = "https:";
  override _getClient(): Deno.HttpClient | undefined {
    if (caCerts === null) {
      return undefined;
    }
    if (caCerts !== undefined) {
      return createHttpClient({ caCerts, http2: false });
    }
    // const status = await Deno.permissions.query({
    //   name: "env",
    //   variable: "NODE_EXTRA_CA_CERTS",
    // });
    // if (status.state !== "granted") {
    //   caCerts = null;
    //   return undefined;
    // }
    const certFilename = Deno.env.get("NODE_EXTRA_CA_CERTS");
    if (!certFilename) {
      caCerts = null;
      return undefined;
    }
    const caCert = Deno.readTextFileSync(certFilename);
    caCerts = [caCert];
    return createHttpClient({ caCerts, http2: false });
  }
}

/** Makes a request to an https server. */
export function request(
  url: string | URL,
  cb?: (res: IncomingMessage) => void,
): HttpsClientRequest;
export function request(
  opts: HttpsRequestOptions,
  cb?: (res: IncomingMessage) => void,
): HttpsClientRequest;
export function request(
  url: string | URL,
  opts: HttpsRequestOptions,
  cb?: (res: IncomingMessage) => void,
): HttpsClientRequest;
// deno-lint-ignore no-explicit-any
export function request(...args: any[]) {
  let options = {};

  if (typeof args[0] === "string") {
    const urlStr = args.shift();
    options = urlToHttpOptions(new URL(urlStr));
  } else if (args[0] instanceof URL) {
    options = urlToHttpOptions(args.shift());
  }

  if (args[0] && typeof args[0] !== "function") {
    Object.assign(options, args.shift());
  }

  options._defaultAgent = globalAgent;
  if (options.agent === undefined) {
    if (options.key !== undefined) {
      options._defaultAgent.options.key = options.key;
    }
    if (options.cert !== undefined) {
      options._defaultAgent.options.cert = options.cert;
    }
    if (options.ca !== undefined) {
      options._defaultAgent.options.ca = options.ca;
    }
  } else {
    if (options.key !== undefined) {
      options.agent.options.key = options.key;
    }
    if (options.cert !== undefined) {
      options.agent.options.cert = options.cert;
    }
    if (options.ca !== undefined) {
      options.agent.options.ca = options.ca;
    }
  }
  args.unshift(options);

  return new HttpsClientRequest(args[0], args[1]);
}
export default {
  Agent,
  Server,
  createServer,
  get,
  globalAgent,
  request,
};
