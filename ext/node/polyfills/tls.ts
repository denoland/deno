// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import tlsCommon from "node:_tls_common";
import tlsWrap from "node:_tls_wrap";
import { convertALPNProtocols } from "ext:deno_node/internal/tls_common.js";
import { ERR_INVALID_ARG_VALUE } from "ext:deno_node/internal/errors.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";
import { readTextFileSync } from "ext:deno_fs/30_fs.js";
import * as io from "ext:deno_io/12_io.js";
import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import {
  op_get_env_no_permission_check,
  op_node_get_ca_certificates,
  op_set_default_ca_certificates,
} from "ext:core/ops";
import { primordials } from "ext:core/mod.js";

const {
  ArrayIsArray,
  ArrayPrototypeIncludes,
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
  SafeRegExp,
  StringPrototypeIncludes,
  StringPrototypeReplace,
  StringPrototypeSplit,
  StringPrototypeTrim,
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
let lazyBundledCertificates: string[] | null = null;
let lazySystemCertificates: string[] | null = null;
let lazyExtraCertificates: string[] | null = null;
let lazyDefaultCertificates: string[] | null = null;

function emitNativeCryptoDebug(message: string) {
  const nodeDebugNative = op_get_env_no_permission_check("NODE_DEBUG_NATIVE") ??
    "";
  if (
    !StringPrototypeIncludes(nodeDebugNative, "crypto")
  ) {
    return;
  }
  const bytes = new TextEncoder().encode(`${message}\n`);
  io.stderr.writeSync(bytes);
}

function getPemCertificatesFromFile(path: string): string[] {
  const content = readTextFileSync(path);
  const certificates = [];
  ArrayPrototypeForEach(
    StringPrototypeSplit(content, "-----END CERTIFICATE-----\n"),
    (line) => {
      if (StringPrototypeTrim(line) === "") {
        return;
      }
      ArrayPrototypePush(certificates, `${line}-----END CERTIFICATE-----\n`);
    },
  );
  return certificates;
}

function getBundledCertificates(): string[] {
  if (lazyBundledCertificates === null) {
    lazyBundledCertificates = op_node_get_ca_certificates(
      "bundled",
    ) as string[];
    ObjectFreeze(lazyBundledCertificates);
  }
  return lazyBundledCertificates;
}

function getSystemCertificates(): string[] {
  if (lazySystemCertificates === null) {
    lazySystemCertificates = op_node_get_ca_certificates("system") as string[];
    ObjectFreeze(lazySystemCertificates);
  }
  return lazySystemCertificates;
}

function getExtraCertificates(): string[] {
  if (lazyExtraCertificates === null) {
    const path = op_get_env_no_permission_check("NODE_EXTRA_CA_CERTS");
    lazyExtraCertificates = path ? getPemCertificatesFromFile(path) : [];
    ObjectFreeze(lazyExtraCertificates);
  }
  return lazyExtraCertificates;
}

function getDefaultCertificates(): string[] {
  if (lazyDefaultCertificates !== null) {
    return lazyDefaultCertificates;
  }

  if (lazyRootCertificates !== null) {
    lazyDefaultCertificates = lazyRootCertificates;
    return lazyDefaultCertificates;
  }

  const certs = [];
  const stores = [];
  ArrayPrototypeForEach(
    StringPrototypeSplit(
      op_get_env_no_permission_check("DENO_TLS_CA_STORE") ?? "mozilla",
      ",",
    ),
    (store) => {
      const trimmedStore = StringPrototypeTrim(store);
      if (trimmedStore.length === 0) {
        return;
      }
      ArrayPrototypePush(stores, trimmedStore);
    },
  );
  const useOpenSslCa =
    op_get_env_no_permission_check("DENO_NODE_USE_OPENSSL_CA") ===
      "1";
  const hasMozillaStore = ArrayPrototypeIncludes(stores, "mozilla");
  const hasSystemStore = ArrayPrototypeIncludes(stores, "system");

  if (hasMozillaStore) {
    if (!useOpenSslCa) {
      emitNativeCryptoDebug(
        "Started loading bundled root certificates off-thread",
      );
    }
    ArrayPrototypeForEach(
      getBundledCertificates(),
      (cert) => ArrayPrototypePush(certs, cert),
    );
  }
  if (hasSystemStore) {
    if (!useOpenSslCa && hasMozillaStore) {
      emitNativeCryptoDebug(
        "Started loading system root certificates off-thread",
      );
    }
    ArrayPrototypeForEach(
      getSystemCertificates(),
      (cert) => ArrayPrototypePush(certs, cert),
    );
  }

  const extra = getExtraCertificates();
  if (extra.length > 0 && !useOpenSslCa) {
    emitNativeCryptoDebug("Started loading extra root certificates off-thread");
  }
  ArrayPrototypeForEach(extra, (cert) => ArrayPrototypePush(certs, cert));

  lazyDefaultCertificates = ObjectFreeze(certs);
  lazyRootCertificates = lazyDefaultCertificates;
  return lazyDefaultCertificates;
}

function ensureLazyRootCertificates(target: string[]) {
  if (lazyRootCertificates === null) {
    lazyRootCertificates = getDefaultCertificates();
    // Clear target and repopulate
    target.length = 0;
    ArrayPrototypeForEach(
      lazyRootCertificates,
      // Strip trailing newline to match Node.js format
      (v: string) =>
        ArrayPrototypePush(
          target,
          StringPrototypeReplace(v, new SafeRegExp("\\n$"), ""),
        ),
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

  for (let i = 0; i < certs.length; ++i) {
    const cert = certs[i];
    if (typeof cert !== "string") {
      throw new TypeError(
        "Each certificate in 'certs' must be a string",
      );
    }
  }

  op_set_default_ca_certificates(certs);

  lazyDefaultCertificates = null;
  lazyRootCertificates = null;
}

export function getCACertificates(
  type: "default" | "system" | "bundled" | "extra" = "default",
) {
  if (type !== undefined) {
    validateString(type, "type");
  }

  switch (type) {
    case "default":
      return getDefaultCertificates();
    case "bundled":
      return getBundledCertificates();
    case "system":
      return getSystemCertificates();
    case "extra":
      return getExtraCertificates();
    default:
      throw new ERR_INVALID_ARG_VALUE("type", type);
  }
}

export function createSecurePair() {
  notImplemented("tls.createSecurePair");
}

const defaultExport = {
  CryptoStream,
  SecurePair,
  Server,
  TLSSocket: tlsWrap.TLSSocket,
  checkServerIdentity: tlsWrap.checkServerIdentity,
  connect: tlsWrap.connect,
  createSecureContext: tlsCommon.createSecureContext,
  createSecurePair,
  createServer: tlsWrap.createServer,
  convertALPNProtocols,
  getCiphers,
  getCACertificates,
  setDefaultCACertificates,
  DEFAULT_CIPHERS: tlsWrap.DEFAULT_CIPHERS,
  DEFAULT_ECDH_CURVE,
  DEFAULT_MAX_VERSION,
  DEFAULT_MIN_VERSION,
  CLIENT_RENEG_LIMIT,
  CLIENT_RENEG_WINDOW,
};
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

export const checkServerIdentity = tlsWrap.checkServerIdentity;
export const connect = tlsWrap.connect;
export const createSecureContext = tlsCommon.createSecureContext;
export const createServer = tlsWrap.createServer;
export const DEFAULT_CIPHERS = tlsWrap.DEFAULT_CIPHERS;
export const TLSSocket = tlsWrap.TLSSocket;
