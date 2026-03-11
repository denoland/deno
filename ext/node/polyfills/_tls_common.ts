// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

import { crypto as cryptoConstants } from "ext:deno_node/internal_binding/constants.ts";
import { kEmptyObject } from "ext:deno_node/internal/util.mjs";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { toBuf } from "ext:deno_node/internal/crypto/util.ts";
import {
  createPrivateKey,
  createPublicKey,
} from "ext:deno_node/internal/crypto/keys.ts";
import { X509Certificate } from "ext:deno_node/internal/crypto/x509.ts";
import { Buffer } from "node:buffer";
import { readFileSync } from "node:fs";
import process from "node:process";
import {
  codes,
  ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED,
  ERR_TLS_INVALID_PROTOCOL_METHOD,
  ERR_TLS_INVALID_PROTOCOL_VERSION,
  ERR_TLS_PROTOCOL_VERSION_CONFLICT,
} from "ext:deno_node/internal/errors.ts";
import {
  validateBuffer,
  validateInt32,
  validateObject,
  validateString,
} from "ext:deno_node/internal/validators.mjs";

const DEFAULT_CIPHERS = cryptoConstants.defaultCoreCipherList;
const DEFAULT_ECDH_CURVE = "auto";
const DEFAULT_MIN_VERSION = "TLSv1.2";
const DEFAULT_MAX_VERSION = "TLSv1.3";
let extraCACerts: string[] | undefined;

function loadExtraCACerts() {
  if (extraCACerts !== undefined) {
    return extraCACerts;
  }

  const path = process.env.NODE_EXTRA_CA_CERTS;
  if (!path) {
    extraCACerts = [];
    return extraCACerts;
  }

  try {
    extraCACerts = [readFileSync(path, "utf8")];
  } catch (err) {
    let message = err instanceof Error ? err.message : String(err);
    if ((err as { code?: string })?.code === "ENOENT") {
      message = "No such file or directory";
    }
    process.emitWarning(
      `Ignoring extra certs from ${path}, load failed: ${message}`,
    );
    extraCACerts = [];
  }

  return extraCACerts;
}

const KNOWN_CIPHER_TOKENS = new Set([
  ...DEFAULT_CIPHERS.split(":"),
  "AES128-GCM-SHA256",
  "AES128-SHA",
  "AES128-SHA256",
  "AES256-GCM-SHA384",
  "AES256-SHA",
  "AES256-SHA256",
  "ECDHE-ECDSA-CHACHA20-POLY1305",
  "ECDHE-ECDSA-AES128-GCM-SHA256",
  "ECDHE-ECDSA-AES256-GCM-SHA384",
  "ECDHE-RSA-CHACHA20-POLY1305",
  "ECDHE-RSA-AES128-GCM-SHA256",
  "ECDHE-RSA-AES256-GCM-SHA384",
  "TLS_AES_128_CCM_8_SHA256",
  "TLS_AES_128_CCM_SHA256",
  "TLS_AES_128_GCM_SHA256",
  "TLS_AES_256_GCM_SHA384",
  "TLS_CHACHA20_POLY1305_SHA256",
]);

const KNOWN_CIPHER_SELECTORS = new Set([
  "ALL",
  "DEFAULT",
  "HIGH",
  "RSA",
]);

function toV(which: string, version: string | undefined, def: string): number {
  const value = version ?? def;
  switch (value) {
    case "TLSv1":
      return cryptoConstants.TLS1_VERSION;
    case "TLSv1.1":
      return cryptoConstants.TLS1_1_VERSION;
    case "TLSv1.2":
      return cryptoConstants.TLS1_2_VERSION;
    case "TLSv1.3":
      return cryptoConstants.TLS1_3_VERSION;
    default:
      throw new ERR_TLS_INVALID_PROTOCOL_VERSION(value, which);
  }
}

function getDefaultCiphers() {
  return DEFAULT_CIPHERS;
}

function getDefaultEcdhCurve() {
  return DEFAULT_ECDH_CURVE;
}

function getProtocolRange(secureProtocol?: string) {
  if (!secureProtocol) {
    return {
      minVersion: DEFAULT_MIN_VERSION,
      maxVersion: DEFAULT_MAX_VERSION,
    };
  }

  switch (secureProtocol) {
    case "SSLv2_method":
    case "SSLv2_server_method":
    case "SSLv2_client_method":
      throw new ERR_TLS_INVALID_PROTOCOL_METHOD("SSLv2 methods disabled");
    case "SSLv3_method":
    case "SSLv3_server_method":
    case "SSLv3_client_method":
      throw new ERR_TLS_INVALID_PROTOCOL_METHOD("SSLv3 methods disabled");
    case "SSLv23_method":
    case "SSLv23_server_method":
    case "SSLv23_client_method":
      return {
        minVersion: DEFAULT_MIN_VERSION,
        maxVersion: "TLSv1.2",
      };
    case "TLS_method":
    case "TLS_server_method":
    case "TLS_client_method":
      return {
        minVersion: DEFAULT_MIN_VERSION,
        maxVersion: DEFAULT_MAX_VERSION,
      };
    case "TLSv1_method":
    case "TLSv1_server_method":
    case "TLSv1_client_method":
      return { minVersion: "TLSv1", maxVersion: "TLSv1" };
    case "TLSv1_1_method":
    case "TLSv1_1_server_method":
    case "TLSv1_1_client_method":
      return { minVersion: "TLSv1.1", maxVersion: "TLSv1.1" };
    case "TLSv1_2_method":
    case "TLSv1_2_server_method":
    case "TLSv1_2_client_method":
      return { minVersion: "TLSv1.2", maxVersion: "TLSv1.2" };
    default:
      throw new ERR_TLS_INVALID_PROTOCOL_METHOD(
        `Unknown method: ${secureProtocol}`,
      );
  }
}

function validateKeyOrCertOption(name: string, value: unknown) {
  if (typeof value !== "string" && !isArrayBufferView(value)) {
    throw new codes.ERR_INVALID_ARG_TYPE(
      name,
      [
        "string",
        "Buffer",
        "TypedArray",
        "DataView",
      ],
      value,
    );
  }
}

function validateKeyOrCertArray(name: string, value: unknown) {
  if (Array.isArray(value)) {
    for (const item of value) {
      validateKeyOrCertOption(name, item);
    }
    return;
  }

  validateKeyOrCertOption(name, value);
}

function validateKeyEntries(name: string, value: unknown, passphrase: unknown) {
  if (Array.isArray(value)) {
    for (const item of value) {
      const pem = item?.pem !== undefined ? item.pem : item;
      const localPassphrase = item?.passphrase !== undefined
        ? item.passphrase
        : passphrase;
      validateKeyOrCertOption(`${name}.key`, pem);
      if (localPassphrase !== undefined && localPassphrase !== null) {
        validateString(localPassphrase, `${name}.passphrase`);
      }
    }
    return;
  }

  validateKeyOrCertOption(`${name}.key`, value);
  if (passphrase !== undefined && passphrase !== null) {
    validateString(passphrase, `${name}.passphrase`);
  }
}

function validateCipherToken(token: string) {
  if (token.startsWith("@SECLEVEL=")) {
    return;
  }
  if (KNOWN_CIPHER_SELECTORS.has(token)) {
    return;
  }
  const selector = token.split("@", 1)[0];
  if (
    selector !== token &&
    KNOWN_CIPHER_SELECTORS.has(selector) &&
    /@SECLEVEL=\d+$/.test(token)
  ) {
    return;
  }
  if (KNOWN_CIPHER_TOKENS.has(token)) {
    return;
  }

  const err = new Error("error:0A0000B9:SSL routines::no cipher match");
  (err as Error & { code?: string }).code = "ERR_SSL_NO_CIPHER_MATCH";
  throw err;
}

function processCiphers(ciphers: string, name: string) {
  const entries = (ciphers || getDefaultCiphers()).split(":");
  const cipherListEntries = [];
  const cipherSuitesEntries = [];

  for (const entry of entries) {
    if (entry.length === 0) {
      continue;
    }
    validateCipherToken(entry);
    if (entry.startsWith("TLS_") || entry.startsWith("!TLS_")) {
      cipherSuitesEntries.push(entry);
    } else {
      cipherListEntries.push(entry);
    }
  }

  const cipherList = cipherListEntries.join(":");
  const cipherSuites = cipherSuitesEntries.join(":");

  if (cipherList === "" && cipherSuites === "") {
    throw new codes.ERR_INVALID_ARG_VALUE(name, entries);
  }

  return { cipherList, cipherSuites };
}

function normalizeCertValue(value: any) {
  if (Array.isArray(value)) {
    return value.map(normalizeCertValue);
  }
  if (typeof value === "string") {
    return value;
  }
  if (isArrayBufferView(value)) {
    return toBuf(value as any).toString();
  }
  return value;
}

function normalizeKeyValue(value: any) {
  if (Array.isArray(value)) {
    return value.map((entry) => {
      if (entry?.pem !== undefined) {
        return {
          ...entry,
          pem: normalizeKeyValue(entry.pem),
        };
      }
      return normalizeKeyValue(entry);
    });
  }
  if (typeof value === "string") {
    return value;
  }
  if (isArrayBufferView(value)) {
    return toBuf(value as any).toString();
  }
  return value;
}

function normalizeCAValue(value: any) {
  return normalizeCertValue(value);
}

function getLeafCertValue(value: any): string | Buffer | undefined {
  if (Array.isArray(value)) {
    return getLeafCertValue(value[0]);
  }
  if (typeof value === "string") {
    return value;
  }
  if (isArrayBufferView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  return undefined;
}

function* iterateKeyEntries(value: any, passphrase: unknown) {
  if (Array.isArray(value)) {
    for (const item of value) {
      if (item?.pem !== undefined) {
        yield {
          key: item.pem,
          passphrase: item?.passphrase !== undefined
            ? item.passphrase
            : passphrase,
        };
      } else {
        yield { key: item, passphrase };
      }
    }
    return;
  }

  yield { key: value, passphrase };
}

function assertKeyMatchesCert(cert: any, key: any, passphrase: unknown) {
  const leafCert = getLeafCertValue(cert);
  if (!leafCert) {
    return;
  }

  const certPublicKey = new X509Certificate(leafCert).publicKey.export({
    format: "der",
    type: "spki",
  });

  for (const entry of iterateKeyEntries(key, passphrase)) {
    const privateKey = createPrivateKey({
      key: entry.key,
      format: "pem",
      passphrase: entry.passphrase,
    });
    const keyPublic = createPublicKey(privateKey).export({
      format: "der",
      type: "spki",
    });

    if (Buffer.compare(certPublicKey, keyPublic) !== 0) {
      throw new Error(
        "error:05800074:x509 certificate routines::key values mismatch",
      );
    }
  }
}

export function configSecureContext(
  context: Record<string, any>,
  options: any = kEmptyObject,
  name = "options",
) {
  validateObject(options, name);

  const {
    allowPartialTrustChain,
    ca,
    cert,
    ciphers = getDefaultCiphers(),
    clientCertEngine,
    crl,
    dhparam,
    ecdhCurve = getDefaultEcdhCurve(),
    key,
    passphrase,
    pfx,
    privateKeyIdentifier,
    privateKeyEngine,
    sessionIdContext,
    sessionTimeout,
    sigalgs,
    ticketKeys,
  } = options;

  if (ciphers !== undefined && ciphers !== null) {
    validateString(ciphers, `${name}.ciphers`);
  }

  const { cipherList, cipherSuites } = processCiphers(
    ciphers,
    `${name}.ciphers`,
  );

  if (ca) {
    validateKeyOrCertArray(`${name}.ca`, ca);
    context.ca = normalizeCAValue(ca);
  }

  if (cert) {
    validateKeyOrCertArray(`${name}.cert`, cert);
    context.cert = normalizeCertValue(cert);
  }

  if (key) {
    validateKeyEntries(name, key, passphrase);
    if (cert) {
      assertKeyMatchesCert(cert, key, passphrase);
    }
    context.key = normalizeKeyValue(key);
  }

  if (sigalgs !== undefined && sigalgs !== null) {
    validateString(sigalgs, `${name}.sigalgs`);
    if (sigalgs === "") {
      throw new codes.ERR_INVALID_ARG_VALUE(`${name}.sigalgs`, sigalgs);
    }
  }

  validateString(ecdhCurve, `${name}.ecdhCurve`);

  if (dhparam !== undefined && dhparam !== null) {
    validateKeyOrCertOption(`${name}.dhparam`, dhparam);
  }

  if (crl !== undefined && crl !== null) {
    validateKeyOrCertArray(`${name}.crl`, crl);
  }

  if (sessionIdContext !== undefined && sessionIdContext !== null) {
    validateString(sessionIdContext, `${name}.sessionIdContext`);
  }

  if (ticketKeys !== undefined && ticketKeys !== null) {
    validateBuffer(ticketKeys, `${name}.ticketKeys`);
    if (ticketKeys.byteLength !== 48) {
      throw new codes.ERR_INVALID_ARG_VALUE(
        `${name}.ticketKeys`,
        ticketKeys.byteLength,
        "must be exactly 48 bytes",
      );
    }
  }

  if (sessionTimeout !== undefined && sessionTimeout !== null) {
    validateInt32(sessionTimeout, `${name}.sessionTimeout`, 0);
  }

  if (typeof clientCertEngine === "string") {
    throw new ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED();
  } else if (clientCertEngine !== undefined && clientCertEngine !== null) {
    throw new codes.ERR_INVALID_ARG_TYPE(
      `${name}.clientCertEngine`,
      ["string", "null", "undefined"],
      clientCertEngine,
    );
  }

  context.allowPartialTrustChain = !!allowPartialTrustChain;
  context.ciphers = ciphers;
  context.cipherList = cipherList;
  context.cipherSuites = cipherSuites;
  context.clientCertEngine = clientCertEngine;
  context.crl = crl;
  context.dhparam = dhparam;
  context.ecdhCurve = ecdhCurve;
  context.passphrase = passphrase;
  context.pfx = pfx;
  context.privateKeyIdentifier = privateKeyIdentifier;
  context.privateKeyEngine = privateKeyEngine;
  context.sessionIdContext = sessionIdContext;
  context.sessionTimeout = sessionTimeout;
  context.sigalgs = sigalgs;
  context.ticketKeys = ticketKeys;
}

export class SecureContext {
  context: Record<string, any>;
  singleUse: boolean;

  constructor(
    secureProtocol?: string,
    secureOptions?: number,
    minVersion?: string,
    maxVersion?: string,
  ) {
    let resolvedMinVersion = minVersion;
    let resolvedMaxVersion = maxVersion;

    if (secureProtocol) {
      if (minVersion != null) {
        throw new ERR_TLS_PROTOCOL_VERSION_CONFLICT(
          minVersion,
          secureProtocol,
        );
      }
      if (maxVersion != null) {
        throw new ERR_TLS_PROTOCOL_VERSION_CONFLICT(
          maxVersion,
          secureProtocol,
        );
      }

      const protocolRange = getProtocolRange(secureProtocol);
      resolvedMinVersion = protocolRange.minVersion;
      resolvedMaxVersion = protocolRange.maxVersion;
    }

    const context = {
      maxProto: toV("maximum", resolvedMaxVersion, DEFAULT_MAX_VERSION),
      maxVersion: resolvedMaxVersion ?? DEFAULT_MAX_VERSION,
      minProto: toV("minimum", resolvedMinVersion, DEFAULT_MIN_VERSION),
      minVersion: resolvedMinVersion ?? DEFAULT_MIN_VERSION,
      secureOptions,
      secureProtocol,
    };
    context.addCACert = (cert: unknown) => {
      validateKeyOrCertOption("cert", cert);
      const normalized = normalizeCAValue(cert);
      if (context.ca === undefined) {
        context.ca = normalized;
      } else if (Array.isArray(context.ca)) {
        context.ca.push(normalized);
      } else {
        context.ca = [context.ca, normalized];
      }
    };
    this.context = context;
    this.singleUse = false;
  }
}

export function createSecureContext(options: any) {
  options ??= kEmptyObject;
  const {
    honorCipherOrder,
    minVersion,
    maxVersion,
    secureProtocol,
  } = options;

  let { secureOptions = 0 } = options;
  if (secureOptions !== undefined && secureOptions !== null) {
    validateInt32(secureOptions, "secureOptions");
  }

  if (honorCipherOrder) {
    secureOptions |= cryptoConstants.SSL_OP_CIPHER_SERVER_PREFERENCE;
  }

  const secureContext = new SecureContext(
    secureProtocol,
    secureOptions,
    minVersion,
    maxVersion,
  );
  configSecureContext(secureContext.context, options);

  const extraCerts = loadExtraCACerts();
  for (const cert of extraCerts) {
    secureContext.context.addCACert(cert);
  }

  return secureContext;
}

export function translatePeerCertificate(c: any) {
  if (!c) {
    return null;
  }

  if (c.issuerCertificate != null) {
    if (c.issuerCertificate === c) {
      c.issuerCertificate = c;
    } else {
      c.issuerCertificate = translatePeerCertificate(c.issuerCertificate);
    }
  }

  if (typeof c.infoAccess === "string") {
    const info = c.infoAccess;
    c.infoAccess = { __proto__: null };

    info.replace(/([^\n:]*):([^\n]*)(?:\n|$)/g, (_all, key, value) => {
      const normalized = value.charCodeAt(0) === 0x22
        ? JSON.parse(value)
        : value;
      if (key in c.infoAccess) {
        c.infoAccess[key].push(normalized);
      } else {
        c.infoAccess[key] = [normalized];
      }
      return "";
    });
  }

  return c;
}

export default {
  SecureContext,
  createSecureContext,
  translatePeerCertificate,
};
