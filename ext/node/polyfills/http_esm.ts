// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
// Re-export `globalAgent` from `_http_agent` so consumers of `node:http` get
// the live binding, matching Node's behavior where `http.globalAgent` updates
// after `setGlobalAgent` propagate to importers.
import { globalAgent } from "node:_http_agent";

const mod = core.loadExtScript("ext:deno_node/http.ts");

export const _connectionListener = mod._connectionListener;
export const Agent = mod.Agent;
export const ClientRequest = mod.ClientRequest;
export const createServer = mod.createServer;
export const get = mod.get;
export { globalAgent };
export const IncomingMessage = mod.IncomingMessage;
export const maxHeaderSize = mod.maxHeaderSize;
export const METHODS = mod.METHODS;
export const OutgoingMessage = mod.OutgoingMessage;
export const request = mod.request;
export const Server = mod.Server;
export const ServerImpl = mod.ServerImpl;
export const ServerResponse = mod.ServerResponse;
export const setMaxIdleHTTPParsers = mod.setMaxIdleHTTPParsers;
export const STATUS_CODES = mod.STATUS_CODES;
export const validateHeaderName = mod.validateHeaderName;
export const validateHeaderValue = mod.validateHeaderValue;

export default mod;
