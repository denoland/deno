// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

(function () {
  const { core } = globalThis.__bootstrap;
  const {
    ERR_INVALID_ARG_TYPE,
    ERR_TLS_INVALID_PROTOCOL_VERSION,
    ERR_TLS_INVALID_PROTOCOL_METHOD,
    ERR_TLS_PROTOCOL_VERSION_CONFLICT,
  } = core.loadExtScript("ext:deno_node/internal/errors.ts");
  const { isArrayBufferView } = core.loadExtScript(
    "ext:deno_node/internal/util/types.ts",
  );
  const { validateString } = core.loadExtScript(
    "ext:deno_node/internal/validators.mjs",
  );
  const { op_node_validate_crl, op_node_validate_pfx } = core.ops;
  const { createPrivateKey } = core.loadExtScript(
    "ext:deno_node/internal/crypto/keys.ts",
  );

  // OpenSSL cipher names are uppercase alphanumeric with hyphens/underscores,
  // "=" (for @SECLEVEL=N), and "+" (for intersection like PSK+HIGH). Examples:
  // "ECDHE-RSA-AES128-GCM-SHA256", "AECDH-NULL-SHA", "@SECLEVEL=2", "PSK+HIGH".
  // Meta-keywords like "ALL", "HIGH", "DEFAULT" also match. We reject strings
  // where no colon-separated entry looks like a valid cipher name, which
  // catches typos like "no-such-cipher".
  const CIPHER_NAME_RE = /^[!+\-@]?[A-Z0-9][A-Z0-9_=+\-]*$/;

  // OpenSSL meta-keywords that select groups of ciphers. Accepting these
  // without an exact match in our known-cipher set lets callers pass the
  // usual "HIGH:!aNULL:..." style cipher lists.
  const META_KEYWORDS: Set<string> = new Set([
    "ALL",
    "COMPLEMENTOFALL",
    "COMPLEMENTOFDEFAULT",
    "DEFAULT",
    "HIGH",
    "MEDIUM",
    "LOW",
    "EXP",
    "EXPORT",
    "EXPORT40",
    "EXPORT56",
    "eNULL",
    "NULL",
    "aNULL",
    "kRSA",
    "aRSA",
    "RSA",
    "kEDH",
    "kDHE",
    "DH",
    "DHE",
    "EDH",
    "ADH",
    "AECDH",
    "ECDH",
    "ECDHE",
    "EECDH",
    "aDSS",
    "DSS",
    "aECDSA",
    "ECDSA",
    "AES",
    "AESGCM",
    "AESCCM",
    "AESCCM8",
    "CAMELLIA",
    "CHACHA20",
    "3DES",
    "DES",
    "RC4",
    "RC2",
    "IDEA",
    "SEED",
    "MD5",
    "SHA1",
    "SHA",
    "SHA256",
    "SHA384",
    "TLSv1",
    "TLSv1.0",
    "TLSv1.2",
    "SSLv3",
    "PSK",
    "kPSK",
    "aPSK",
    "kECDHEPSK",
    "kDHEPSK",
    "kRSAPSK",
    "aGOST",
    "aGOST01",
    "kGOST",
    "GOST94",
    "GOST89MAC",
    "STRENGTH",
    "FIPS",
    "SUITEB128",
    "SUITEB128ONLY",
    "SUITEB192",
  ]);

  // Known OpenSSL-style cipher names that map to TLS 1.3 / 1.2 suites we
  // understand. Keep this in sync with `cipherMap` in tls.ts.
  const KNOWN_CIPHERS: Set<string> = new Set([
    "AES128-GCM-SHA256",
    "AES256-GCM-SHA384",
    "AES128-SHA",
    "AES256-SHA",
    "AES128-SHA256",
    "AES256-SHA256",
    "DES-CBC3-SHA",
    "DHE-RSA-AES128-GCM-SHA256",
    "DHE-RSA-AES256-GCM-SHA384",
    "DHE-RSA-AES128-SHA",
    "DHE-RSA-AES128-SHA256",
    "DHE-RSA-AES256-SHA",
    "DHE-RSA-AES256-SHA256",
    "ECDHE-ECDSA-AES128-GCM-SHA256",
    "ECDHE-ECDSA-AES256-GCM-SHA384",
    "ECDHE-ECDSA-CHACHA20-POLY1305",
    "ECDHE-ECDSA-AES128-SHA",
    "ECDHE-ECDSA-AES128-SHA256",
    "ECDHE-ECDSA-AES256-SHA",
    "ECDHE-ECDSA-AES256-SHA384",
    "ECDHE-RSA-AES128-GCM-SHA256",
    "ECDHE-RSA-AES256-GCM-SHA384",
    "ECDHE-RSA-CHACHA20-POLY1305",
    "ECDHE-RSA-AES128-SHA",
    "ECDHE-RSA-AES128-SHA256",
    "ECDHE-RSA-AES256-SHA",
    "ECDHE-RSA-AES256-SHA384",
    "PSK-AES128-CBC-SHA",
    "PSK-AES256-CBC-SHA",
    "PSK-AES128-GCM-SHA256",
    "PSK-AES256-GCM-SHA384",
    "PSK-CHACHA20-POLY1305",
    "TLS_AES_128_CCM_8_SHA256",
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
  ]);

  function isKnownCipherName(name: string): boolean {
    if (name === "") return false;
    if (KNOWN_CIPHERS.has(name)) return true;
    if (META_KEYWORDS.has(name)) return true;
    // Real OpenSSL cipher names are KX-AUTH-ENC-MAC tuples joined with '-'
    // (e.g. AECDH-NULL-SHA, ECDHE-RSA-AES128-GCM-SHA256).  Accept any
    // strictly-uppercase name that contains a hyphen so we don't have to keep
    // an exhaustive cipher table.  This still rejects shapeless names like
    // "FOOBARBAZ" and mixed-case strings like "TLS_not_a_cipher".
    if (name.includes("-")) {
      let allUpper = true;
      for (let i = 0; i < name.length; i++) {
        const ch = name.charCodeAt(i);
        if (ch >= 97 /* a */ && ch <= 122 /* z */) {
          allUpper = false;
          break;
        }
      }
      if (allUpper) return true;
    }
    return false;
  }

  function isAcceptedCipher(entry: string): boolean {
    if (entry === "") return false;
    // '@' prefixes control directives like "@SECLEVEL=N" or "@STRENGTH" which
    // never refer to a specific cipher: skip them entirely.
    if (entry.charCodeAt(0) === 64 /* @ */) return false;
    let name = entry;
    const first = name.charCodeAt(0);
    // OpenSSL list operators '+', '-', '!' select/deselect a group.
    if (first === 43 /* + */ || first === 45 /* - */ || first === 33 /* ! */) {
      name = name.slice(1);
    }
    // OpenSSL '+' inside an entry is the intersection operator (e.g. "PSK+HIGH"
    // selects ciphers in PSK AND HIGH).  Accept if every part is known.
    if (name.includes("+")) {
      const parts = name.split("+");
      for (const part of parts) {
        if (!isKnownCipherName(part)) return false;
      }
      return true;
    }
    return isKnownCipherName(name);
  }

  function validateCipherList(ciphers: string): void {
    // Allow empty string ("" means "use default ciphers" per Node.js docs).
    if (ciphers === "") return;

    const entries = ciphers.split(":");
    let nonEmptyEntries = 0;
    let hasValidShape = false;
    let hasAcceptedCipher = false;
    for (const entry of entries) {
      if (entry === "") continue;
      nonEmptyEntries++;
      if (CIPHER_NAME_RE.test(entry)) {
        hasValidShape = true;
      }
      if (isAcceptedCipher(entry)) {
        hasAcceptedCipher = true;
        break;
      }
    }

    // A list of only colons / whitespace is a Node-level argument error rather
    // than a SSL configuration error: distinguish those two cases.
    if (nonEmptyEntries === 0) {
      const err = new Error(
        `The argument 'ciphers' must be a non-empty string. Received '${ciphers}'`,
      ) as any;
      err.code = "ERR_INVALID_ARG_VALUE";
      throw err;
    }

    // Two-tier rejection mirrors Node + OpenSSL: shape-only failures (e.g.
    // lowercase) are obviously wrong; shape-valid strings still fail when
    // they reference no cipher we understand.
    if (!hasValidShape || !hasAcceptedCipher) {
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
      // SSLv2 / SSLv3 methods are accepted as names by OpenSSL but the actual
      // protocols are disabled. Node reports a specific error message for
      // each so we mirror that.
      if (
        /^SSLv2(?:_(?:client|server))?_method$/.test(options.secureProtocol)
      ) {
        const err = new Error(
          `${options.secureProtocol} is no longer supported. ` +
            `SSLv2 methods disabled.`,
        );
        throw err;
      }
      if (
        /^SSLv3(?:_(?:client|server))?_method$/.test(options.secureProtocol)
      ) {
        const err = new Error(
          `${options.secureProtocol} is no longer supported. ` +
            `SSLv3 methods disabled.`,
        );
        throw err;
      }
      const range = kProtocolMap[options.secureProtocol];
      if (!range) {
        throw new ERR_TLS_INVALID_PROTOCOL_METHOD(
          options.secureProtocol,
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

  class SecureContext {
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
        // OpenSSL engines are not supported in Deno (which uses rustls).
        // Match Node's behaviour when OpenSSL fails to load the engine: throw
        // an Error whose message contains "could not load the shared library"
        // and carries an `opensslErrorStack` array.
        const err = new Error(
          `error:25066067:DSO support routines:dlfcn_load:could not load the shared library`,
        ) as any;
        err.opensslErrorStack = [
          `error:25070067:DSO support routines:DSO_load:could not load the shared library`,
          `error:260B6084:engine routines:dynamic_load:dso not found`,
        ];
        err.library = "DSO support routines";
        err.function = "dlfcn_load";
        err.reason = "could not load the shared library";
        err.code = "ERR_OSSL_DSO_COULD_NOT_LOAD_THE_SHARED_LIBRARY";
        throw err;
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

  function createSecureContext(options: any = {}) {
    return new SecureContext(options);
  }

  function translatePeerCertificate(c: any) {
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

  return {
    SecureContext,
    createSecureContext,
    translatePeerCertificate,
    default: {
      SecureContext,
      createSecureContext,
      translatePeerCertificate,
    },
  };
})();
