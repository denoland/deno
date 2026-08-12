// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/_tls_wrap.js");

export const checkServerIdentity = mod.checkServerIdentity;
export const connect = mod.connect;
export const createServer = mod.createServer;
export const DEFAULT_CIPHERS = mod.DEFAULT_CIPHERS;
export const Server = mod.Server;
export const TLSSocket = mod.TLSSocket;
export const unfqdn = mod.unfqdn;

export default mod.default;
