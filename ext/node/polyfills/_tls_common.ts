// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

import {
  ERR_TLS_INVALID_PROTOCOL_VERSION,
  ERR_TLS_PROTOCOL_VERSION_CONFLICT,
} from "ext:deno_node/internal/errors.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";

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
