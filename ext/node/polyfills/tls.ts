// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import tlsCommon from "node:_tls_common";
import tlsWrap from "node:_tls_wrap";
import {
  op_get_root_certificates,
  op_set_default_ca_certificates,
} from "ext:core/ops";
import { primordials } from "ext:core/mod.js";

const {
  ArrayIsArray,
  ArrayPrototypeForEach,
  ArrayPrototypePush,
  ObjectKeys,
  ObjectFreeze,
  Proxy,
  ReflectDefineProperty,
  ReflectDeleteProperty,
  ReflectGet,
  ReflectGetOwnPropertyDescriptor,
  ReflectHas,
  ReflectIsExtensible,
  ReflectOwnKeys,
  ReflectPreventExtensions,
  ReflectSet,
  SafeArrayIterator,
  StringPrototypeToLowerCase,
  TypeError,
} = primordials;

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
  return ArrayPrototypeMap(
    ObjectKeys(cipherMap),
    (name) => StringPrototypeToLowerCase(name),
  );
}

let lazyRootCertificates: string[] | null = null;
function ensureLazyRootCertificates(target: string[]) {
  if (lazyRootCertificates === null) {
    lazyRootCertificates = op_get_root_certificates() as string[];
    // Clear target and repopulate
    target.length = 0;
    ArrayPrototypeForEach(
      lazyRootCertificates,
      (v) => ArrayPrototypePush(target, v),
    );
    ObjectFreeze(target);
  }
}
export const rootCertificates = new Proxy([] as string[], {
  // @ts-ignore __proto__ is not in the types
  __proto__: null,
  get(target, prop) {
    ensureLazyRootCertificates(target);
    return ReflectGet(target, prop);
  },
  ownKeys(target) {
    ensureLazyRootCertificates(target);
    return ReflectOwnKeys(target);
  },
  has(target, prop) {
    ensureLazyRootCertificates(target);
    return ReflectHas(target, prop);
  },
  getOwnPropertyDescriptor(target, prop) {
    ensureLazyRootCertificates(target);
    return ReflectGetOwnPropertyDescriptor(target, prop);
  },
  set(target, prop, value) {
    ensureLazyRootCertificates(target);
    return ReflectSet(target, prop, value);
  },
  defineProperty(target, prop, descriptor) {
    ensureLazyRootCertificates(target);
    return ReflectDefineProperty(target, prop, descriptor);
  },
  deleteProperty(target, prop) {
    ensureLazyRootCertificates(target);
    return ReflectDeleteProperty(target, prop);
  },
  isExtensible(target) {
    ensureLazyRootCertificates(target);
    return ReflectIsExtensible(target);
  },
  preventExtensions(target) {
    ensureLazyRootCertificates(target);
    return ReflectPreventExtensions(target);
  },
  setPrototypeOf() {
    return false;
  },
});

export const DEFAULT_ECDH_CURVE = "auto";
export const DEFAULT_MAX_VERSION = "TLSv1.3";
export const DEFAULT_MIN_VERSION = "TLSv1.2";
export const CLIENT_RENEG_LIMIT = 3;
export const CLIENT_RENEG_WINDOW = 600;

export class CryptoStream {}
export class SecurePair {}
export const Server = tlsWrap.Server;

export function setDefaultCACertificates(certs: string[]) {
  if (!ArrayIsArray(certs)) {
    throw new TypeError(
      "The argument 'certs' must be an array of strings",
    );
  }

  for (const cert of new SafeArrayIterator(certs)) {
    if (typeof cert !== "string") {
      throw new TypeError(
        "Each certificate in 'certs' must be a string",
      );
    }
  }

  op_set_default_ca_certificates(certs);

  lazyRootCertificates = null;
}

export function createSecurePair() {
  notImplemented("tls.createSecurePair");
}

export default {
  CryptoStream,
  SecurePair,
  Server,
  TLSSocket: tlsWrap.TLSSocket,
  checkServerIdentity: tlsWrap.checkServerIdentity,
  connect: tlsWrap.connect,
  createSecureContext: tlsCommon.createSecureContext,
  createSecurePair,
  createServer: tlsWrap.createServer,
  getCiphers,
  rootCertificates,
  setDefaultCACertificates,
  DEFAULT_CIPHERS: tlsWrap.DEFAULT_CIPHERS,
  DEFAULT_ECDH_CURVE,
  DEFAULT_MAX_VERSION,
  DEFAULT_MIN_VERSION,
  CLIENT_RENEG_LIMIT,
  CLIENT_RENEG_WINDOW,
};

export const checkServerIdentity = tlsWrap.checkServerIdentity;
export const connect = tlsWrap.connect;
export const createSecureContext = tlsCommon.createSecureContext;
export const createServer = tlsWrap.createServer;
export const DEFAULT_CIPHERS = tlsWrap.DEFAULT_CIPHERS;
export const TLSSocket = tlsWrap.TLSSocket;
