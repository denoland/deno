// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { notImplemented } from "internal:deno_node/polyfills/_utils.ts";
import _tls_common from "internal:deno_node/polyfills/_tls_common.ts";
import _tls_wrap from "internal:deno_node/polyfills/_tls_wrap.ts";

// openssl -> rustls
const cipherMap = {
  "__proto__": null,
  "AES128-GCM-SHA256": "TLS13_AES_128_GCM_SHA256",
  "AES256-GCM-SHA384": "TLS13_AES_256_GCM_SHA384",
  "ECDHE-ECDSA-AES128-GCM-SHA256": "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
  "ECDHE-ECDSA-AES256-GCM-SHA384": "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
  "ECDHE-ECDSA-CHACHA20-POLY1305":
    "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
  "ECDHE-RSA-AES128-GCM-SHA256": "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
  "ECDHE-RSA-AES256-GCM-SHA384": "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
  "ECDHE-RSA-CHACHA20-POLY1305": "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
  "TLS_AES_128_GCM_SHA256": "TLS13_AES_128_GCM_SHA256",
  "TLS_AES_256_GCM_SHA384": "TLS13_AES_256_GCM_SHA384",
  "TLS_CHACHA20_POLY1305_SHA256": "TLS13_CHACHA20_POLY1305_SHA256",
};

export function getCiphers() {
  // TODO(bnoordhuis) Use locale-insensitive toLowerCase()
  return Object.keys(cipherMap).map((name) => name.toLowerCase());
}

export const rootCertificates = undefined;
export const DEFAULT_ECDH_CURVE = "auto";
export const DEFAULT_MAX_VERSION = "TLSv1.3";
export const DEFAULT_MIN_VERSION = "TLSv1.2";

export class CryptoStream {}
export class SecurePair {}
export const Server = _tls_wrap.Server;
export function createSecurePair() {
  notImplemented("tls.createSecurePair");
}

export default {
  CryptoStream,
  SecurePair,
  Server,
  TLSSocket: _tls_wrap.TLSSocket,
  checkServerIdentity: _tls_wrap.checkServerIdentity,
  connect: _tls_wrap.connect,
  createSecureContext: _tls_common.createSecureContext,
  createSecurePair,
  createServer: _tls_wrap.createServer,
  getCiphers,
  rootCertificates,
  DEFAULT_CIPHERS: _tls_wrap.DEFAULT_CIPHERS,
  DEFAULT_ECDH_CURVE,
  DEFAULT_MAX_VERSION,
  DEFAULT_MIN_VERSION,
};

export const checkServerIdentity = _tls_wrap.checkServerIdentity;
export const connect = _tls_wrap.connect;
export const createSecureContext = _tls_common.createSecureContext;
export const createServer = _tls_wrap.createServer;
export const DEFAULT_CIPHERS = _tls_wrap.DEFAULT_CIPHERS;
export const TLSSocket = _tls_wrap.TLSSocket;
