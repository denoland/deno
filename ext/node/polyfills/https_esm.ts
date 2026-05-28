// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
// See http_esm.ts: force node:process bootstrap before any https code can
// run, so test-sanitizer-pending ops from the deferred bootstrap are
// observed as pre-test state.
import "node:process";
const mod = core.loadExtScript("ext:deno_node/https.ts");

export const Agent = mod.Agent;
export const Server = mod.Server;
export const createServer = mod.createServer;
export const get = mod.get;
export const globalAgent = mod.globalAgent;
export const request = mod.request;

export default mod;
