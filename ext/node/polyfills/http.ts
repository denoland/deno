// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any

import {
  OutgoingMessage,
  validateHeaderName,
  validateHeaderValue,
} from "node:_http_outgoing";
import { ClientRequest } from "node:_http_client";
import {
  Agent,
  globalAgent,
  setGlobalAgent,
} from "node:_http_agent";
import { IncomingMessage } from "node:_http_incoming";
import {
  _connectionListener,
  Server as ServerImpl,
  ServerResponse,
  STATUS_CODES,
} from "node:_http_server";
import { primordials } from "ext:core/mod.js";
const { ArrayPrototypeSlice, ArrayPrototypeSort } = primordials;
import { methods, parsers } from "node:_http_common";
import { validateInteger } from "ext:deno_node/internal/validators.mjs";
const METHODS = ArrayPrototypeSort(ArrayPrototypeSlice(methods));

export interface RequestOptions {
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

export type ServerHandler = (req: any, res: any) => void;

export function createServer(opts: any, requestListener?: ServerHandler) {
  return new ServerImpl(opts, requestListener);
}

/** Makes an HTTP request. */
export function request(
  url: string | URL,
  cb?: (res: any) => void,
): ClientRequest;
export function request(
  opts: RequestOptions,
  cb?: (res: any) => void,
): ClientRequest;
export function request(
  url: string | URL,
  opts: RequestOptions,
  cb?: (res: any) => void,
): ClientRequest;

export function request(...args: any[]) {
  return new ClientRequest(args[0], args[1], args[2]);
}

/** Makes a `GET` HTTP request. */
export function get(
  url: string | URL,
  cb?: (res: any) => void,
): ClientRequest;
export function get(
  opts: RequestOptions,
  cb?: (res: any) => void,
): ClientRequest;
export function get(
  url: string | URL,
  opts: RequestOptions,
  cb?: (res: any) => void,
): ClientRequest;

export function get(...args: any[]) {
  const req = request(args[0], args[1], args[2]);
  req.end();
  return req;
}

// Default max header size matches Node.js default (16 KiB).
// Node reads this from --max-http-header-size; we hardcode it.
export const maxHeaderSize = 16_384;

export function setMaxIdleHTTPParsers(max: number) {
  validateInteger(max, "max", 1);
  parsers.max = max;
}

export {
  _connectionListener,
  Agent,
  ClientRequest,
  globalAgent,
  IncomingMessage,
  METHODS,
  OutgoingMessage,
  ServerImpl,
  ServerImpl as Server,
  ServerResponse,
  STATUS_CODES,
  validateHeaderName,
  validateHeaderValue,
};
export default {
  _connectionListener,
  Agent,
  get globalAgent() {
    return globalAgent;
  },
  set globalAgent(value) {
    setGlobalAgent(value);
  },
  ClientRequest,
  STATUS_CODES,
  METHODS,
  createServer,
  Server: ServerImpl,
  IncomingMessage,
  OutgoingMessage,
  ServerResponse,
  request,
  get,
  validateHeaderName,
  validateHeaderValue,
  setMaxIdleHTTPParsers,
  maxHeaderSize,
};
