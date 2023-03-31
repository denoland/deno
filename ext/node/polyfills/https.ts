// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import { urlToHttpOptions } from "ext:deno_node/internal/url.ts";
import {
  ClientRequest,
  IncomingMessageForClient as IncomingMessage,
  type RequestOptions,
} from "ext:deno_node/http.ts";
import { Agent as HttpAgent } from "ext:deno_node/_http_agent.mjs";
import { createHttpClient } from "ext:deno_fetch/22_http_client.js";
import { readTextFileSync } from "ext:deno_fs/30_fs.js";
import { env } from "ext:runtime/30_os.js";

export class Server {
  constructor() {
    notImplemented("https.Server.prototype.constructor");
  }
}
export function createServer() {
  notImplemented("https.createServer");
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
}

const globalAgent = new Agent({
  keepAlive: true,
  scheduling: "lifo",
  timeout: 5000,
});

/** HttpsClientRequest class loosely follows http.ClientRequest class API. */
class HttpsClientRequest extends ClientRequest {
  override defaultProtocol = "https:";
  override _getClient(): Deno.HttpClient | undefined {
    if (caCerts === null) {
      return undefined;
    }
    if (caCerts !== undefined) {
      return createHttpClient({ caCerts, http2: false });
    }
    // const status = await permissions.query({
    //   name: "env",
    //   variable: "NODE_EXTRA_CA_CERTS",
    // });
    // if (status.state !== "granted") {
    //   caCerts = null;
    //   return undefined;
    // }
    const certFilename = env.get("NODE_EXTRA_CA_CERTS");
    if (!certFilename) {
      caCerts = null;
      return undefined;
    }
    const caCert = readTextFileSync(certFilename);
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
