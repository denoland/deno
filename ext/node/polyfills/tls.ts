// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-lint-ignore-file prefer-primordials

import { notImplemented } from "ext:deno_node/_utils.ts";
import tlsCommon from "node:_tls_common";
import tlsWrap from "node:_tls_wrap";
import { Buffer } from "node:buffer";
import process from "node:process";
import {
  op_get_root_certificates,
  op_set_default_ca_certificates,
} from "ext:core/ops";
import { codes } from "ext:deno_node/internal/errors.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";
import { primordials } from "ext:core/mod.js";

const {
  ArrayIsArray,
  ArrayPrototypeForEach,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ObjectDefineProperty,
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
  StringPrototypeToLowerCase,
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

let bundledRootCertificates: string[] | null = null;
function ensureBundledRootCertificates(target: string[]) {
  if (bundledRootCertificates === null) {
    bundledRootCertificates = op_get_root_certificates() as string[];
    // Clear target and repopulate
    target.length = 0;
    ArrayPrototypeForEach(
      bundledRootCertificates,
      (v) => ArrayPrototypePush(target, v),
    );
    ObjectFreeze(target);
  }
}
export const rootCertificates = new Proxy([] as string[], {
  // @ts-ignore __proto__ is not in the types
  __proto__: null,
  get(target, prop) {
    ensureBundledRootCertificates(target);
    return ReflectGet(target, prop);
  },
  ownKeys(target) {
    ensureBundledRootCertificates(target);
    return ReflectOwnKeys(target);
  },
  has(target, prop) {
    ensureBundledRootCertificates(target);
    return ReflectHas(target, prop);
  },
  getOwnPropertyDescriptor(target, prop) {
    ensureBundledRootCertificates(target);
    return ReflectGetOwnPropertyDescriptor(target, prop);
  },
  set(target, prop, value) {
    ensureBundledRootCertificates(target);
    return ReflectSet(target, prop, value);
  },
  defineProperty(target, prop, descriptor) {
    ensureBundledRootCertificates(target);
    return ReflectDefineProperty(target, prop, descriptor);
  },
  deleteProperty(target, prop) {
    ensureBundledRootCertificates(target);
    return ReflectDeleteProperty(target, prop);
  },
  isExtensible(target) {
    ensureBundledRootCertificates(target);
    return ReflectIsExtensible(target);
  },
  preventExtensions(target) {
    ensureBundledRootCertificates(target);
    return ReflectPreventExtensions(target);
  },
  setPrototypeOf() {
    return false;
  },
});

// Reuse the loadExtraCACerts from _tls_common which has proper error
// handling (emits a warning instead of throwing on read failure).
let extraCACertificates: string[] | undefined;
function cacheExtraCACertificates() {
  if (extraCACertificates) {
    return extraCACertificates;
  }
  extraCACertificates = ObjectFreeze(tlsCommon.loadExtraCACerts());
  return extraCACertificates;
}

let systemCACertificates: string[] | undefined;
function cacheSystemCACertificates() {
  systemCACertificates ??= ObjectFreeze([]);
  return systemCACertificates;
}

let defaultCACertificates: string[] | undefined;
let customDefaultCACertificates: string[] | undefined;
let hasResetDefaultCACertificates = false;

function cacheDefaultCACertificates() {
  if (defaultCACertificates) {
    return defaultCACertificates;
  }

  if (hasResetDefaultCACertificates) {
    defaultCACertificates = ObjectFreeze(customDefaultCACertificates ?? []);
    return defaultCACertificates;
  }

  defaultCACertificates = [
    ...rootCertificates,
    ...cacheExtraCACertificates(),
  ];
  ObjectFreeze(defaultCACertificates);
  return defaultCACertificates;
}

export const DEFAULT_ECDH_CURVE = "auto";
export const DEFAULT_MAX_VERSION = "TLSv1.3";
export const DEFAULT_MIN_VERSION = "TLSv1.2";
export const CLIENT_RENEG_LIMIT = 3;
export const CLIENT_RENEG_WINDOW = 600;

export class CryptoStream {}
export class SecurePair {}
export const Server = tlsWrap.Server;

export function getCACertificates(type = "default") {
  validateString(type, "type");

  switch (type) {
    case "default":
      return cacheDefaultCACertificates();
    case "bundled":
      return rootCertificates;
    case "system":
      return cacheSystemCACertificates();
    case "extra":
      return cacheExtraCACertificates();
    default:
      throw new codes.ERR_INVALID_ARG_VALUE("type", type);
  }
}

function normalizeCACert(cert: string | ArrayBufferView, index: number) {
  if (typeof cert === "string") {
    return cert;
  }
  if (isArrayBufferView(cert)) {
    return Buffer.from(cert.buffer, cert.byteOffset, cert.byteLength)
      .toString();
  }
  throw new codes.ERR_INVALID_ARG_TYPE(
    `certs[${index}]`,
    ["string", "ArrayBufferView"],
    cert,
  );
}

export function setDefaultCACertificates(
  certs: Array<string | ArrayBufferView>,
) {
  if (!ArrayIsArray(certs)) {
    throw new codes.ERR_INVALID_ARG_TYPE("certs", "Array", certs);
  }

  const normalizedCerts = [];
  const seen = new Set<string>();
  for (let i = 0; i < certs.length; i++) {
    const cert = normalizeCACert(certs[i], i);
    if (!seen.has(cert)) {
      seen.add(cert);
      ArrayPrototypePush(normalizedCerts, cert);
    }
  }

  op_set_default_ca_certificates(normalizedCerts);
  customDefaultCACertificates = ObjectFreeze([...normalizedCerts]);
  defaultCACertificates = undefined;
  hasResetDefaultCACertificates = true;
}

export function createSecurePair() {
  notImplemented("tls.createSecurePair");
}

// Imported from shared helper (avoids circular dep with _tls_wrap.js)
import { convertALPNProtocols } from "ext:deno_node/internal/tls_common.js";
export { convertALPNProtocols };

const exported = {
  CryptoStream,
  SecurePair,
  Server,
  TLSSocket: tlsWrap.TLSSocket,
  checkServerIdentity: tlsWrap.checkServerIdentity,
  connect: tlsWrap.connect,
  convertALPNProtocols,
  createSecureContext: tlsCommon.createSecureContext,
  createSecurePair,
  createServer: tlsWrap.createServer,
  getCACertificates,
  getCiphers,
  setDefaultCACertificates,
  DEFAULT_CIPHERS: tlsWrap.DEFAULT_CIPHERS,
  DEFAULT_ECDH_CURVE,
  DEFAULT_MAX_VERSION,
  DEFAULT_MIN_VERSION,
  CLIENT_RENEG_LIMIT,
  CLIENT_RENEG_WINDOW,
};

ObjectDefineProperty(exported, "rootCertificates", {
  __proto__: null,
  configurable: false,
  enumerable: true,
  get: () => rootCertificates,
});

export default exported;

export const checkServerIdentity = tlsWrap.checkServerIdentity;
export const connect = tlsWrap.connect;
export const createSecureContext = tlsCommon.createSecureContext;
export const createServer = tlsWrap.createServer;
export const DEFAULT_CIPHERS = tlsWrap.DEFAULT_CIPHERS;
export const TLSSocket = tlsWrap.TLSSocket;
