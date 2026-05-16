// Copyright 2018-2026 the Deno authors. MIT license.
// Side-effect imports: net.ts uses `createLazyLoader` for these at module
// top level, which requires them to already be loaded by the time net.ts
// IIFE runs. Without this, eager importers of `node:net` (the `_http_*`
// modules) hit a TDZ on stream/dns exports during 01_require.js
// instantiation.
import "node:stream";
import "node:dns";
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/net.ts");

export const BlockList = mod.BlockList;
export const isIP = mod.isIP;
export const isIPv4 = mod.isIPv4;
export const isIPv6 = mod.isIPv6;
export const SocketAddress = mod.SocketAddress;
export const _createServerHandle = mod._createServerHandle;
export const _normalizeArgs = mod._normalizeArgs;
export const connect = mod.connect;
export const createConnection = mod.createConnection;
export const createServer = mod.createServer;
export const getDefaultAutoSelectFamily = mod.getDefaultAutoSelectFamily;
export const getDefaultAutoSelectFamilyAttemptTimeout =
  mod.getDefaultAutoSelectFamilyAttemptTimeout;
export const Server = mod.Server;
export const setDefaultAutoSelectFamily = mod.setDefaultAutoSelectFamily;
export const setDefaultAutoSelectFamilyAttemptTimeout =
  mod.setDefaultAutoSelectFamilyAttemptTimeout;
export const Socket = mod.Socket;
export const Stream = mod.Stream;

export default mod.default;
