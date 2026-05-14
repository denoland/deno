// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import tlsCommon from "node:_tls_common";
import tlsWrap from "node:_tls_wrap";

const mod = core.loadExtScript("ext:deno_node/tls.ts");

const { ObjectDefineProperty } = primordials;

export const CryptoStream = mod.CryptoStream;
export const SecurePair = mod.SecurePair;
export const Server = tlsWrap.Server;
export const TLSSocket = tlsWrap.TLSSocket;
export const checkServerIdentity = tlsWrap.checkServerIdentity;
export const connect = tlsWrap.connect;
export const createSecureContext = tlsCommon.createSecureContext;
export const createServer = tlsWrap.createServer;
export const convertALPNProtocols = mod.convertALPNProtocols;
export const getCiphers = mod.getCiphers;
export const getCACertificates = mod.getCACertificates;
export const setDefaultCACertificates = mod.setDefaultCACertificates;
export const createSecurePair = mod.createSecurePair;
export const rootCertificates = mod.rootCertificates;
export const DEFAULT_CIPHERS = tlsWrap.DEFAULT_CIPHERS;
export const DEFAULT_ECDH_CURVE = mod.DEFAULT_ECDH_CURVE;
export const DEFAULT_MAX_VERSION = mod.DEFAULT_MAX_VERSION;
export const DEFAULT_MIN_VERSION = mod.DEFAULT_MIN_VERSION;
export const CLIENT_RENEG_LIMIT = mod.CLIENT_RENEG_LIMIT;
export const CLIENT_RENEG_WINDOW = mod.CLIENT_RENEG_WINDOW;

let defaultMaxVersionOverride: string | undefined;
let defaultMinVersionOverride: string | undefined;

const defaultExport = {
  CryptoStream,
  SecurePair,
  Server,
  TLSSocket,
  checkServerIdentity,
  connect,
  createSecureContext,
  createSecurePair,
  createServer,
  convertALPNProtocols,
  getCiphers,
  getCACertificates,
  setDefaultCACertificates,
  DEFAULT_CIPHERS,
  DEFAULT_ECDH_CURVE,
  CLIENT_RENEG_LIMIT,
  CLIENT_RENEG_WINDOW,
};
ObjectDefineProperty(defaultExport as any, "DEFAULT_MAX_VERSION", {
  __proto__: null,
  configurable: true,
  enumerable: true,
  get: () => defaultMaxVersionOverride ?? mod.DEFAULT_MAX_VERSION,
  set: (value) => defaultMaxVersionOverride = value,
});
ObjectDefineProperty(defaultExport as any, "DEFAULT_MIN_VERSION", {
  __proto__: null,
  configurable: true,
  enumerable: true,
  get: () => defaultMinVersionOverride ?? mod.DEFAULT_MIN_VERSION,
  set: (value) => defaultMinVersionOverride = value,
});
// Make rootCertificates non-writable so `tls.rootCertificates = X` throws
// TypeError in strict mode (matches Node.js behavior).
// deno-lint-ignore no-explicit-any
ObjectDefineProperty(defaultExport as any, "rootCertificates", {
  __proto__: null,
  configurable: false,
  enumerable: true,
  get: () => rootCertificates,
});

export default defaultExport;
