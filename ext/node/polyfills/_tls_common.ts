// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

import {
  ERR_INVALID_ARG_TYPE,
  ERR_TLS_INVALID_PROTOCOL_VERSION,
  ERR_TLS_PROTOCOL_VERSION_CONFLICT,
} from "ext:deno_node/internal/errors.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";
import { createPrivateKey } from "ext:deno_node/internal/crypto/keys.ts";
import { op_node_validate_crl, op_node_validate_pfx } from "ext:core/ops";

// OpenSSL cipher names are uppercase alphanumeric with hyphens/underscores
// and "=" (for @SECLEVEL=N). Examples: "ECDHE-RSA-AES128-GCM-SHA256",
// "AECDH-NULL-SHA", "@SECLEVEL=2". Meta-keywords like "ALL", "HIGH",
// "DEFAULT" also match. We reject strings where no colon-separated entry
// looks like a valid cipher name, which catches typos like "no-such-cipher".
const CIPHER_NAME_RE = /^[!+\-@]?[A-Z0-9][A-Z0-9_=\-]*$/;

function validateCipherList(ciphers: string): void {
  const entries = ciphers.split(":");
  let hasValidEntry = false;
  for (const entry of entries) {
    if (entry === "") continue;
    if (CIPHER_NAME_RE.test(entry)) {
      hasValidEntry = true;
      break;
    }
  }
  if (!hasValidEntry) {
    const err = new Error("no cipher match") as any;
    err.code = "ERR_SSL_NO_CIPHER_MATCH";
    throw err;
  }
}

// Mirrors OpenSSL's SECLEVEL bit-strength requirements (see
// `ssl_security_default_callback` in OpenSSL). OpenSSL 3 defaults `DEFAULT`
// to SECLEVEL=1 for the cipher list itself, but loading a key/cert checks
// against SECLEVEL=2 unless the cipher list lowers it. We mirror that by
// defaulting to 2 here so small RSA keys are rejected when no level is given.
function parseSecLevel(ciphers: unknown): number {
  if (typeof ciphers !== "string") return 2;
  const re = /(?:^|:)@SECLEVEL=(\d+)(?=$|:)/;
  const match = re.exec(ciphers);
  if (!match) return 2;
  const n = globalThis.parseInt(match[1], 10);
  if (Number.isNaN(n) || n < 0) return 2;
  return n;
}

function rsaMinBitsForSecLevel(level: number): number {
  if (level <= 0) return 0;
  if (level === 1) return 1024;
  if (level === 2) return 2048;
  if (level === 3) return 3072;
  if (level === 4) return 7680;
  return 15360;
}

function ecMinBitsForSecLevel(level: number): number {
  if (level <= 0) return 0;
  if (level === 1) return 160;
  if (level === 2) return 224;
  if (level === 3) return 256;
  if (level === 4) return 384;
  return 512;
}

function ecCurveBits(name: string | undefined): number {
  switch (name) {
    case "prime256v1":
      return 256;
    case "secp224r1":
      return 224;
    case "secp384r1":
      return 384;
    case "secp521r1":
      return 521;
    case "secp256k1":
      return 256;
    default:
      return 0;
  }
}

function throwKeyTooSmall(): never {
  const err = new Error(
    "error:0A00018A:SSL routines::key too small",
  ) as any;
  err.code = "ERR_OSSL_SSL_KEY_TOO_SMALL";
  err.library = "SSL routines";
  err.reason = "key too small";
  throw err;
}

// Validates each PEM/DER private key against the security level implied by
// `options.ciphers`. Mirrors OpenSSL's behaviour so that Node-compat callers
// can rely on `SECLEVEL=0` to permit weak keys (and the default SECLEVEL to
// reject them).
function validateKeyStrength(options: any): void {
  if (!options.key) return;
  const level = parseSecLevel(options.ciphers);
  if (level <= 0) return;

  const keys = globalThis.Array.isArray(options.key)
    ? options.key
    : [options.key];
  for (const k of keys) {
    if (!k) continue;
    let keyData: any = k;
    let passphrase: any = options.passphrase;
    if (
      typeof k === "object" && !isArrayBufferView(k) &&
      !(k instanceof globalThis.ArrayBuffer)
    ) {
      keyData = (k as any).pem ?? (k as any).key;
      passphrase = (k as any).passphrase ?? passphrase;
    }
    if (!keyData) continue;

    let keyObject;
    try {
      keyObject = passphrase != null
        ? createPrivateKey({ key: keyData, passphrase })
        : createPrivateKey(keyData);
    } catch {
      // Don't mask the underlying parse error - let the TLS layer surface it.
      continue;
    }

    const type = keyObject.asymmetricKeyType;
    const details = keyObject.asymmetricKeyDetails as any;
    let bits = 0;
    let minBits = 0;

    if (type === "rsa" || type === "rsa-pss" || type === "dsa") {
      bits = (details && details.modulusLength) || 0;
      minBits = rsaMinBitsForSecLevel(level);
    } else if (type === "ec") {
      bits = ecCurveBits(details && details.namedCurve);
      minBits = ecMinBitsForSecLevel(level);
    } else {
      continue;
    }

    if (bits > 0 && bits < minBits) {
      throwKeyTooSmall();
    }
  }
}

// Map legacy secureProtocol strings to [minVersion, maxVersion] pairs.
// Node.js maps these in src/crypto/crypto_context.cc.
const kProtocolMap: Record<string, [string, string]> = {
  "__proto__": null as any,
  "TLSv1_method": ["TLSv1", "TLSv1"],
  "TLSv1_client_method": ["TLSv1", "TLSv1"],
  "TLSv1_server_method": ["TLSv1", "TLSv1"],
  "TLSv1_1_method": ["TLSv1.1", "TLSv1.1"],
  "TLSv1_1_client_method": ["TLSv1.1", "TLSv1.1"],
  "TLSv1_1_server_method": ["TLSv1.1", "TLSv1.1"],
  "TLSv1_2_method": ["TLSv1.2", "TLSv1.2"],
  "TLSv1_2_client_method": ["TLSv1.2", "TLSv1.2"],
  "TLSv1_2_server_method": ["TLSv1.2", "TLSv1.2"],
  "SSLv23_method": ["TLSv1", "TLSv1.3"],
  "SSLv23_client_method": ["TLSv1", "TLSv1.3"],
  "SSLv23_server_method": ["TLSv1", "TLSv1.3"],
  "TLS_method": ["TLSv1", "TLSv1.3"],
  "TLS_client_method": ["TLSv1", "TLSv1.3"],
  "TLS_server_method": ["TLSv1", "TLSv1.3"],
};

const kValidVersions: Set<string> = new Set([
  "TLSv1",
  "TLSv1.1",
  "TLSv1.2",
  "TLSv1.3",
]);

function toStringOrUndefined(val: any): string | undefined {
  if (val == null) return undefined;
  if (typeof val === "string") return val;
  if (isArrayBufferView(val) || val instanceof globalThis.ArrayBuffer) {
    return new TextDecoder().decode(val);
  }
  return `${val}`;
}

function normalizeCertValue(
  val: any,
): string | string[] | undefined {
  if (val == null) return undefined;
  if (globalThis.Array.isArray(val)) {
    return val.map((v: any) => toStringOrUndefined(v)!).filter(
      globalThis.Boolean,
    );
  }
  return toStringOrUndefined(val);
}

function getProtocolRange(
  options: any,
): { minVersion: string; maxVersion: string } {
  let minVersion = "TLSv1.2"; // Default per Node.js
  let maxVersion = "TLSv1.3";

  if (options.secureProtocol) {
    const range = kProtocolMap[options.secureProtocol];
    if (!range) {
      throw new ERR_TLS_INVALID_PROTOCOL_VERSION(
        options.secureProtocol,
        "secureProtocol",
      );
    }

    // If secureProtocol is set, minVersion/maxVersion must not also be set
    if (options.minVersion || options.maxVersion) {
      throw new ERR_TLS_PROTOCOL_VERSION_CONFLICT(
        options.minVersion || options.maxVersion,
        "secureProtocol",
      );
    }

    [minVersion, maxVersion] = range;
  } else {
    if (options.minVersion) {
      if (!kValidVersions.has(options.minVersion)) {
        throw new ERR_TLS_INVALID_PROTOCOL_VERSION(
          options.minVersion,
          "minVersion",
        );
      }
      minVersion = options.minVersion;
    }
    if (options.maxVersion) {
      if (!kValidVersions.has(options.maxVersion)) {
        throw new ERR_TLS_INVALID_PROTOCOL_VERSION(
          options.maxVersion,
          "maxVersion",
        );
      }
      maxVersion = options.maxVersion;
    }
  }

  return { minVersion, maxVersion };
}

function isValidKeyCertValue(val: any): boolean {
  return typeof val === "string" ||
    isArrayBufferView(val) ||
    val instanceof globalThis.ArrayBuffer;
}

function validateKeyCertOption(
  val: any,
  name: string,
  allowKeyObjects: boolean,
) {
  if (!val) return; // falsy values (false, null, undefined, 0, '') are skipped
  if (isValidKeyCertValue(val)) return;
  if (globalThis.Array.isArray(val)) {
    for (let i = 0; i < val.length; i++) {
      const item = val[i];
      if (!item) continue;
      if (isValidKeyCertValue(item)) continue;
      // For key, objects like { pem, passphrase } are allowed inside arrays
      if (
        allowKeyObjects && typeof item === "object" && item !== null
      ) continue;
      throw new ERR_INVALID_ARG_TYPE(
        name,
        ["string", "Buffer", "TypedArray", "DataView"],
        item,
      );
    }
    return;
  }
  throw new ERR_INVALID_ARG_TYPE(
    name,
    ["string", "Buffer", "TypedArray", "DataView"],
    val,
  );
}

function toUint8Array(val: any): Uint8Array {
  if (typeof val === "string") {
    return new TextEncoder().encode(val);
  }
  if (val instanceof globalThis.ArrayBuffer) {
    return new Uint8Array(val);
  }
  if (isArrayBufferView(val)) {
    return new Uint8Array(val.buffer, val.byteOffset, val.byteLength);
  }
  return new TextEncoder().encode(String(val));
}

const secureContextBrand = new WeakSet<object>();

export class SecureContext {
  context: {
    ca?: string | string[];
    cert?: string;
    key?: string;
    minVersion: string;
    maxVersion: string;
    ciphers?: string;
    passphrase?: string;
    sigalgs?: string;
    ecdhCurve?: string;
  };

  constructor(options: any = {}) {
    if (options.ciphers != null) {
      validateString(options.ciphers, "options.ciphers");
      validateCipherList(options.ciphers);
    }
    if (options.key && options.passphrase != null) {
      validateString(options.passphrase, "options.passphrase");
    }
    if (options.clientCertEngine != null) {
      validateString(options.clientCertEngine, "options.clientCertEngine");
    }
    if (options.privateKeyEngine != null) {
      validateString(options.privateKeyEngine, "options.privateKeyEngine");
    }
    if (options.privateKeyIdentifier != null) {
      validateString(
        options.privateKeyIdentifier,
        "options.privateKeyIdentifier",
      );
    }
    if (options.ecdhCurve != null) {
      validateString(options.ecdhCurve, "options.ecdhCurve");
    }
    // Validate cert before key - Node.js processes cert first (SetCert before SetKey)
    validateKeyCertOption(options.cert, "options.cert", false);
    validateKeyCertOption(options.key, "options.key", true);
    validateKeyCertOption(options.ca, "options.ca", false);

    // Validate PFX / PKCS#12 data.
    if (options.pfx != null) {
      const pfxData = toUint8Array(options.pfx);
      const pfxPassphrase = options.passphrase != null
        ? String(options.passphrase)
        : null;
      op_node_validate_pfx(pfxData, pfxPassphrase);
    }

    // Validate CRL data.
    if (options.crl != null) {
      const crls = globalThis.Array.isArray(options.crl)
        ? options.crl
        : [options.crl];
      for (const crl of crls) {
        op_node_validate_crl(toUint8Array(crl));
      }
    }

    // Mirror OpenSSL's SECLEVEL key-size enforcement. The cipher list is
    // applied before the key is loaded (matching Node.js' order in
    // `SecureContext::Init`), so `DEFAULT:@SECLEVEL=0` retains compatibility
    // with weak keys while `DEFAULT` rejects them.
    validateKeyStrength(options);

    const { minVersion, maxVersion } = getProtocolRange(options);

    this.context = {
      ca: normalizeCertValue(options.ca),
      cert: toStringOrUndefined(options.cert),
      key: toStringOrUndefined(options.key),
      minVersion,
      maxVersion,
      ciphers: options.ciphers,
      passphrase: options.passphrase,
      sigalgs: options.sigalgs,
      ecdhCurve: options.ecdhCurve,
    };
    secureContextBrand.add(this.context);
    Object.defineProperty(this.context, "_external", {
      __proto__: null,
      configurable: true,
      enumerable: false,
      get(this: object) {
        if (!secureContextBrand.has(this)) {
          throw new TypeError("Illegal invocation");
        }
        return this;
      },
    });
    (this.context as any).setOptions = function setOptions(
      this: object,
      _options?: number,
    ) {
      if (!secureContextBrand.has(this)) {
        throw new TypeError("Illegal invocation");
      }
    };
  }

  // Backward compat: current _tls_wrap.js accesses .ca, .cert, .key directly
  get ca() {
    return this.context.ca;
  }
  get cert() {
    return this.context.cert;
  }
  get key() {
    return this.context.key;
  }
}

export function createSecureContext(options: any = {}) {
  return new SecureContext(options);
}

export function translatePeerCertificate(c: any) {
  if (!c) {
    return null;
  }

  if (c.issuerCertificate != null) {
    if (c.issuerCertificate === c) {
      // Self-signed root CA: issuer is itself. Intentional self-assignment
      // to preserve the circular reference (matches Node.js behavior).
      c.issuerCertificate = c;
    } else {
      c.issuerCertificate = translatePeerCertificate(c.issuerCertificate);
    }
  }

  if (typeof c.infoAccess === "string") {
    const info = c.infoAccess;
    c.infoAccess = { __proto__: null };

    info.replace(
      /([^\n:]*):([^\n]*)(?:\n|$)/g,
      (_all: string, key: string, value: string) => {
        const normalized = value.charCodeAt(0) === 0x22
          ? JSON.parse(value)
          : value;
        if (key in c.infoAccess) {
          c.infoAccess[key].push(normalized);
        } else {
          c.infoAccess[key] = [normalized];
        }
        return "";
      },
    );
  }

  return c;
}

export default {
  SecureContext,
  createSecureContext,
  translatePeerCertificate,
};
