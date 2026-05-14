// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { ArrayPrototypeSlice, ArrayPrototypeSort } = primordials;

const {
  OutgoingMessage,
  validateHeaderName,
  validateHeaderValue,
} = core.createLazyLoader("node:_http_outgoing")();
const { ClientRequest } = core.createLazyLoader("node:_http_client")();
const httpAgent = core.createLazyLoader("node:_http_agent")();
const { Agent } = httpAgent;
const { IncomingMessage } = core.createLazyLoader("node:_http_incoming")();
const {
  _connectionListener,
  Server: ServerImpl,
  ServerResponse,
  STATUS_CODES,
} = core.createLazyLoader("node:_http_server")();
const { methods, parsers } = core.createLazyLoader("node:_http_common")();
const { validateInteger } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const METHODS = ArrayPrototypeSort(ArrayPrototypeSlice(methods));

interface RequestOptions {
  agent?: Agent;
  auth?: string;
  createConnection?: () => unknown;
  defaultPort?: number;
  family?: number;
  headers?: Record<string, string>;
  hints?: number;
  host?: string;
  hostname?: string;
  insecureHTTPParser?: boolean;
  localAddress?: string;
  lookup?: () => void;
  maxHeaderSize?: number;
  method?: string;
  path?: string;
  port?: number;
  protocol?: string;
  setHost?: boolean;
  socketPath?: string;
  timeout?: number;
  signal?: AbortSignal;
  href?: string;
}

type ServerHandler = (req: any, res: any) => void;

function createServer(opts: any, requestListener?: ServerHandler) {
  return new ServerImpl(opts, requestListener);
}

function request(...args: any[]) {
  return new ClientRequest(args[0], args[1], args[2]);
}

function get(...args: any[]) {
  const req = request(args[0], args[1], args[2]);
  req.end();
  return req;
}

// Default max header size matches Node.js default (16 KiB).
// Node reads this from --max-http-header-size; we hardcode it.
const maxHeaderSize = 16_384;

function setMaxIdleHTTPParsers(max: number) {
  validateInteger(max, "max", 1);
  parsers.max = max;
}

return {
  _connectionListener,
  Agent,
  ClientRequest,
  createServer,
  get,
  get globalAgent() {
    return httpAgent.globalAgent;
  },
  set globalAgent(value) {
    httpAgent.setGlobalAgent(value);
  },
  IncomingMessage,
  maxHeaderSize,
  METHODS,
  OutgoingMessage,
  request,
  Server: ServerImpl,
  ServerImpl,
  ServerResponse,
  setMaxIdleHTTPParsers,
  STATUS_CODES,
  validateHeaderName,
  validateHeaderValue,
};
})();
