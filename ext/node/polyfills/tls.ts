// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { notImplemented } = core.loadExtScript("ext:deno_node/_utils.ts");
const { getExecArgvOptionValue, getOptionValue } = core.loadExtScript(
  "ext:deno_node/internal/options.ts",
);
const { convertALPNProtocols } = core.loadExtScript(
  "ext:deno_node/internal/tls_common.js",
);
const { X509Certificate } = core.loadExtScript(
  "ext:deno_node/internal/crypto/x509.ts",
);
const {
  op_get_env_no_permission_check,
  op_get_root_certificates,
  op_node_get_ca_certificates,
  op_set_default_ca_certificates,
} = core.ops;
const {
  validateOneOf,
  validateString,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { isArrayBufferView } = core.loadExtScript(
  "ext:deno_node/internal/util/types.ts",
);
const { ERR_INVALID_ARG_TYPE } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);

const { isTypedArray } = core;
const {
  ArrayIsArray,
  ArrayPrototypeIncludes,
  ArrayPrototypeForEach,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  DataViewPrototypeGetBuffer,
  DataViewPrototypeGetByteLength,
  DataViewPrototypeGetByteOffset,
  Error,
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
  StringPrototypeIncludes,
  StringPrototypeSplit,
  StringPrototypeTrim,
  StringPrototypeToLowerCase,
  SafeSet,
  SetPrototypeAdd,
  SetPrototypeHas,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  Uint8Array,
} = primordials;

// Lazy-init: tls.ts is loaded into the startup snapshot before TextDecoder
// is registered as a global, so a top-level `new TextDecoder()` would throw.
let utf8Decoder: TextDecoder | null = null;

// deno-lint-ignore no-explicit-any
function arrayBufferViewToString(view: any): string {
  // Use the matching prototype getter so a forged accessor on the view
  // can't redirect us to a different ArrayBuffer / out-of-bounds region.
  const isTA = isTypedArray(view);
  const buffer = isTA
    ? TypedArrayPrototypeGetBuffer(view)
    : DataViewPrototypeGetBuffer(view);
  const byteOffset = isTA
    ? TypedArrayPrototypeGetByteOffset(view)
    : DataViewPrototypeGetByteOffset(view);
  const byteLength = isTA
    ? TypedArrayPrototypeGetByteLength(view)
    : DataViewPrototypeGetByteLength(view);
  if (utf8Decoder === null) {
    utf8Decoder = new TextDecoder("utf-8");
  }
  return utf8Decoder.decode(new Uint8Array(buffer, byteOffset, byteLength));
}

// openssl -> rustls
const cipherMap = {
  "__proto__": null,
  "AES128-GCM-SHA256": "TLS13_AES_128_GCM_SHA256",
  "AES256-GCM-SHA384": "TLS13_AES_256_GCM_SHA384",
  "AES256-SHA": "TLS_RSA_WITH_AES_256_CBC_SHA",
  "ECDHE-ECDSA-AES128-GCM-SHA256": "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
  "ECDHE-ECDSA-AES256-GCM-SHA384": "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
  "ECDHE-ECDSA-CHACHA20-POLY1305":
    "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
  "ECDHE-RSA-AES128-GCM-SHA256": "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
  "ECDHE-RSA-AES256-GCM-SHA384": "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
  "ECDHE-RSA-CHACHA20-POLY1305": "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
  "TLS_AES_128_CCM_8_SHA256": "TLS13_AES_128_CCM_8_SHA256",
  "TLS_AES_128_GCM_SHA256": "TLS13_AES_128_GCM_SHA256",
  "TLS_AES_256_GCM_SHA384": "TLS13_AES_256_GCM_SHA384",
  "TLS_CHACHA20_POLY1305_SHA256": "TLS13_CHACHA20_POLY1305_SHA256",
};

function getCiphers() {
  // TODO(bnoordhuis) Use locale-insensitive toLowerCase()
  return ArrayPrototypeMap(
    ObjectKeys(cipherMap),
    (name) => StringPrototypeToLowerCase(name),
  );
}

let lazyRootCertificates: string[] | null = null;
const cachedCACertificates: Record<string, string[]> = {
  __proto__: null as unknown as string[],
};

function writeNativeCryptoDebug(message: string) {
  const nodeDebugNative = op_get_env_no_permission_check("NODE_DEBUG_NATIVE") ??
    "";
  if (
    !StringPrototypeIncludes(nodeDebugNative, "crypto")
  ) {
    return;
  }
  core.print(`${message}\n`, true);
}

function emitDefaultCertificatesDebug() {
  const useOpenSslCa =
    op_get_env_no_permission_check("DENO_NODE_USE_OPENSSL_CA") === "1";
  if (useOpenSslCa) {
    return;
  }

  const stores: string[] = [];
  ArrayPrototypeForEach(
    StringPrototypeSplit(
      op_get_env_no_permission_check("DENO_TLS_CA_STORE") ?? "mozilla",
      ",",
    ),
    (store) => {
      const trimmedStore = StringPrototypeTrim(store);
      if (trimmedStore.length > 0) {
        ArrayPrototypePush(stores, trimmedStore);
      }
    },
  );
  if (ArrayPrototypeIncludes(stores, "mozilla")) {
    writeNativeCryptoDebug(
      "Started loading bundled root certificates off-thread",
    );
  }
  if (ArrayPrototypeIncludes(stores, "system")) {
    writeNativeCryptoDebug(
      "Started loading system root certificates off-thread",
    );
  }
  if (op_get_env_no_permission_check("NODE_EXTRA_CA_CERTS")) {
    writeNativeCryptoDebug(
      "Started loading extra root certificates off-thread",
    );
  }
}

function ensureLazyRootCertificates(target: string[]) {
  if (lazyRootCertificates === null) {
    lazyRootCertificates = op_get_root_certificates() as string[];
    // Clear target and repopulate
    target.length = 0;
    ArrayPrototypeForEach(
      lazyRootCertificates,
      (v: string) => ArrayPrototypePush(target, v),
    );
    ObjectFreeze(target);
  }
}
const rootCertificates = new Proxy([] as string[], {
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

const DEFAULT_ECDH_CURVE = "auto";
const CLIENT_RENEG_LIMIT = 3;
const CLIENT_RENEG_WINDOW = 600;

class CryptoStream {}
class SecurePair {}

function getDefaultMaxVersion(): string {
  if (getOptionValue("--tls-max-v1.3")) return "TLSv1.3";
  if (getOptionValue("--tls-max-v1.2")) return "TLSv1.2";
  return "TLSv1.3";
}

function getDefaultMinVersion(): string {
  if (getOptionValue("--tls-min-v1.0")) return "TLSv1";
  if (getOptionValue("--tls-min-v1.1")) return "TLSv1.1";
  if (getOptionValue("--tls-min-v1.2")) return "TLSv1.2";
  if (getOptionValue("--tls-min-v1.3")) return "TLSv1.3";
  return "TLSv1.2";
}

let hasDefaultCACertificatesOverride = false;

function hasStore(store: string): boolean {
  const storesValue = op_get_env_no_permission_check("DENO_TLS_CA_STORE");
  if (storesValue == null) {
    return false;
  }
  const stores = StringPrototypeSplit(storesValue, ",");
  for (let i = 0; i < stores.length; i++) {
    if (StringPrototypeTrim(stores[i]) === store) {
      return true;
    }
  }
  return false;
}

function getDefaultUsesSystemCertificates(): boolean {
  const useSystemCaExecArgvOption = getExecArgvOptionValue("--use-system-ca");
  if (useSystemCaExecArgvOption !== undefined) {
    return !!useSystemCaExecArgvOption;
  }
  const useSystemCaOption = getOptionValue("--use-system-ca");
  if (
    globalThis.process?.env?.NODE_USE_SYSTEM_CA === "1" ||
    op_get_env_no_permission_check("NODE_USE_SYSTEM_CA") === "1"
  ) {
    return true;
  }
  if (useSystemCaOption === true) {
    return true;
  }
  if (useSystemCaOption === false) {
    return false;
  }
  if (hasStore("system")) {
    return true;
  }
  return false;
}

function getDefaultUsesBundledCertificates(): boolean {
  const storesValue = op_get_env_no_permission_check("DENO_TLS_CA_STORE");
  if (storesValue != null) {
    return hasStore("mozilla");
  }
  if (getOptionValue("--use-openssl-ca")) {
    return false;
  }
  const useBundledCaOption = getOptionValue("--use-bundled-ca");
  return useBundledCaOption !== false;
}

function makeNodeCryptoError(code: string, message: string) {
  const err = new Error(message);
  (err as Error & { code?: string }).code = code;
  return err;
}

function validateDefaultCACertificates(certs: string[]) {
  let validCount = 0;
  for (let i = 0; i < certs.length; i++) {
    try {
      new X509Certificate(certs[i]);
      validCount++;
    } catch {
      if (validCount === 0) {
        throw makeNodeCryptoError(
          "ERR_CRYPTO_OPERATION_FAILED",
          "No valid certificates found in the provided array",
        );
      }
      throw makeNodeCryptoError(
        "ERR_OSSL_PEM_ASN1_LIB",
        "error:0680009B:asn1 encoding routines::too long",
      );
    }
  }
}

function setDefaultCACertificates(
  certs: (string | ArrayBufferView)[],
) {
  if (!ArrayIsArray(certs)) {
    throw new ERR_INVALID_ARG_TYPE("certs", "Array", certs);
  }

  const normalized: string[] = [];
  const seen = new SafeSet();
  for (let i = 0; i < certs.length; ++i) {
    const cert = certs[i];
    let normalizedCert;
    if (typeof cert === "string") {
      normalizedCert = cert;
    } else if (isArrayBufferView(cert)) {
      normalizedCert = arrayBufferViewToString(cert);
    } else {
      throw new ERR_INVALID_ARG_TYPE(
        `certs[${i}]`,
        ["string", "ArrayBufferView"],
        cert,
      );
    }
    if (!SetPrototypeHas(seen, normalizedCert)) {
      SetPrototypeAdd(seen, normalizedCert);
      ArrayPrototypePush(normalized, normalizedCert);
    }
  }

  validateDefaultCACertificates(normalized);
  op_set_default_ca_certificates(normalized);
  hasDefaultCACertificatesOverride = true;

  // The bundled root certificates (`rootCertificates` proxy /
  // `lazyRootCertificates`) come from the webpki Mozilla bundle and don't
  // change here, so don't invalidate them: doing so would re-enter
  // `ensureLazyRootCertificates` and try to mutate a frozen target.
  // Only the 'default' cache reflects what we just wrote.
  ArrayPrototypeForEach(
    ObjectKeys(cachedCACertificates),
    (key) => {
      if (key !== "bundled") {
        delete cachedCACertificates[key];
      }
    },
  );
}

function getCACertificates(type: string = "default"): string[] {
  validateString(type, "type");
  validateOneOf(type, "type", ["default", "system", "bundled", "extra"]);

  if (cachedCACertificates[type] !== undefined) {
    return cachedCACertificates[type];
  }

  let certs: string[];
  if (type === "default" && !hasDefaultCACertificatesOverride) {
    certs = [];
    if (getDefaultUsesBundledCertificates()) {
      ArrayPrototypeForEach(
        getCACertificates("bundled"),
        (cert) => ArrayPrototypePush(certs, cert),
      );
    }
    if (getDefaultUsesSystemCertificates()) {
      ArrayPrototypeForEach(
        getCACertificates("system"),
        (cert) => ArrayPrototypePush(certs, cert),
      );
    }
    ArrayPrototypeForEach(
      getCACertificates("extra"),
      (cert) => ArrayPrototypePush(certs, cert),
    );
    ObjectFreeze(certs);
    emitDefaultCertificatesDebug();
  } else if (type === "bundled") {
    certs = rootCertificates;
  } else {
    certs = ObjectFreeze(op_node_get_ca_certificates(type)) as string[];
    if (type === "default") {
      emitDefaultCertificatesDebug();
    }
  }
  cachedCACertificates[type] = certs;
  return certs;
}

function createSecurePair() {
  notImplemented("tls.createSecurePair");
}

return {
  CryptoStream,
  SecurePair,
  convertALPNProtocols,
  getCiphers,
  getCACertificates,
  setDefaultCACertificates,
  createSecurePair,
  rootCertificates,
  DEFAULT_ECDH_CURVE,
  get DEFAULT_MAX_VERSION() {
    return getDefaultMaxVersion();
  },
  get DEFAULT_MIN_VERSION() {
    return getDefaultMinVersion();
  },
  CLIENT_RENEG_LIMIT,
  CLIENT_RENEG_WINDOW,
};
})();
