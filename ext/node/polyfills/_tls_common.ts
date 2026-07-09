// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayBufferPrototype,
  ArrayIsArray,
  ArrayPrototypeFilter,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  Boolean,
  DataViewPrototypeGetBuffer,
  DataViewPrototypeGetByteLength,
  DataViewPrototypeGetByteOffset,
  Error,
  JSONParse,
  NumberIsNaN,
  NumberParseInt,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  RegExpPrototypeExec,
  RegExpPrototypeTest,
  SafeArrayIterator,
  SafeRegExp,
  SafeSet,
  SafeWeakSet,
  SetPrototypeHas,
  String,
  StringPrototypeCharCodeAt,
  StringPrototypeIncludes,
  StringPrototypeReplace,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  TypeError,
  Uint8Array,
  WeakSetPrototypeHas,
} = primordials;
const {
  ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED,
  ERR_INVALID_ARG_TYPE,
  ERR_TLS_INVALID_PROTOCOL_METHOD,
  ERR_TLS_INVALID_PROTOCOL_VERSION,
  ERR_TLS_PROTOCOL_VERSION_CONFLICT,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { getOptionValue } = core.loadExtScript(
  "ext:deno_node/internal/options.ts",
);
const { isArrayBufferView, isTypedArray } = core.loadExtScript(
  "ext:deno_node/internal/util/types.ts",
);
const { validateString } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { op_node_validate_crl, op_node_load_pfx } = core.ops;
const { createPrivateKey } = core.loadExtScript(
  "ext:deno_node/internal/crypto/keys.ts",
);

// OpenSSL cipher names are uppercase alphanumeric with hyphens/underscores
// and "=" (for @SECLEVEL=N). Examples: "ECDHE-RSA-AES128-GCM-SHA256",
// "AECDH-NULL-SHA", "@SECLEVEL=2". Meta-keywords like "ALL", "HIGH",
// "DEFAULT" also match. We reject strings where no colon-separated entry
// looks like a valid cipher name, which catches typos like "no-such-cipher".
const CIPHER_NAME_RE = new SafeRegExp(
  "^[!+\\-@]?[A-Z0-9][A-Z0-9_=\\-]*(?:@[A-Z0-9_=\\-]+)?$",
);
const CIPHER_META_NAMES = new SafeSet([
  "ALL",
  "COMPLEMENTOFALL",
  "COMPLEMENTOFDEFAULT",
  "DEFAULT",
  "FIPS",
  "HIGH",
  "LOW",
  "MEDIUM",
  "SUITEB128",
  "SUITEB128ONLY",
  "SUITEB192",
]);

function validateCipherList(ciphers: string): void {
  const entries = StringPrototypeSplit(ciphers, ":");
  let hasValidEntry = false;
  for (const entry of new SafeArrayIterator(entries)) {
    if (entry === "") continue;
    if (!RegExpPrototypeTest(CIPHER_NAME_RE, entry)) {
      continue;
    }
    const normalized = StringPrototypeReplace(
      entry,
      new SafeRegExp("^[!+\\-]"),
      "",
    );
    const name = StringPrototypeSplit(normalized, "@", 1)[0];
    if (
      StringPrototypeStartsWith(normalized, "@SECLEVEL=") ||
      SetPrototypeHas(CIPHER_META_NAMES, name) ||
      StringPrototypeStartsWith(name, "TLS_") ||
      StringPrototypeIncludes(name, "-")
    ) {
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
  const re = new SafeRegExp("(?:^|:)@SECLEVEL=(\\d+)(?=$|:)");
  const match = RegExpPrototypeExec(re, ciphers);
  if (!match) return 2;
  const n = NumberParseInt(match[1], 10);
  if (NumberIsNaN(n) || n < 0) return 2;
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

  const keys = ArrayIsArray(options.key) ? options.key : [options.key];
  for (const k of new SafeArrayIterator(keys)) {
    if (!k) continue;
    let keyData: any = k;
    let passphrase: any = options.passphrase;
    if (
      typeof k === "object" && !isArrayBufferView(k) &&
      !ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, k)
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

const kValidVersions: Set<string> = new SafeSet([
  "TLSv1",
  "TLSv1.1",
  "TLSv1.2",
  "TLSv1.3",
]);

function toStringOrUndefined(val: any): string | undefined {
  if (val == null) return undefined;
  if (typeof val === "string") return val;
  if (
    isArrayBufferView(val) ||
    ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, val)
  ) {
    return new TextDecoder().decode(val);
  }
  return `${val}`;
}

function normalizeCertValue(
  val: any,
): string | string[] | undefined {
  if (val == null) return undefined;
  if (ArrayIsArray(val)) {
    return ArrayPrototypeFilter(
      ArrayPrototypeMap(val, (v: any) => toStringOrUndefined(v)!),
      Boolean,
    );
  }
  return toStringOrUndefined(val);
}

function getProtocolRange(
  options: any,
): { minVersion: string; maxVersion: string } {
  let minVersion = getDefaultMinVersion();
  let maxVersion = getDefaultMaxVersion();

  if (options.secureProtocol) {
    // If secureProtocol is set, minVersion/maxVersion must not also be set.
    // Node raises this conflict before validating the protocol method string.
    if (options.minVersion || options.maxVersion) {
      throw new ERR_TLS_PROTOCOL_VERSION_CONFLICT(
        options.minVersion || options.maxVersion,
        "secureProtocol",
      );
    }

    const range = kProtocolMap[options.secureProtocol];
    if (!range) {
      if (
        options.secureProtocol === "SSLv2_method" ||
        options.secureProtocol === "SSLv2_client_method" ||
        options.secureProtocol === "SSLv2_server_method"
      ) {
        throw new ERR_TLS_INVALID_PROTOCOL_METHOD("SSLv2 methods disabled");
      }
      if (
        options.secureProtocol === "SSLv3_method" ||
        options.secureProtocol === "SSLv3_client_method" ||
        options.secureProtocol === "SSLv3_server_method"
      ) {
        throw new ERR_TLS_INVALID_PROTOCOL_METHOD("SSLv3 methods disabled");
      }
      throw new ERR_TLS_INVALID_PROTOCOL_METHOD(
        `Unknown method: ${options.secureProtocol}`,
      );
    }

    minVersion = range[0];
    maxVersion = range[1];
  } else {
    if (options.minVersion) {
      if (!SetPrototypeHas(kValidVersions, options.minVersion)) {
        throw new ERR_TLS_INVALID_PROTOCOL_VERSION(
          options.minVersion,
          "minVersion",
        );
      }
      minVersion = options.minVersion;
    }
    if (options.maxVersion) {
      if (!SetPrototypeHas(kValidVersions, options.maxVersion)) {
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

function getDefaultMinVersion(): string {
  if (getOptionValue("--tls-min-v1.0")) return "TLSv1";
  if (getOptionValue("--tls-min-v1.1")) return "TLSv1.1";
  if (getOptionValue("--tls-min-v1.2")) return "TLSv1.2";
  if (getOptionValue("--tls-min-v1.3")) return "TLSv1.3";
  return "TLSv1.2";
}

function getDefaultMaxVersion(): string {
  if (getOptionValue("--tls-max-v1.3")) return "TLSv1.3";
  if (getOptionValue("--tls-max-v1.2")) return "TLSv1.2";
  return "TLSv1.3";
}

function isValidKeyCertValue(val: any): boolean {
  return typeof val === "string" ||
    isArrayBufferView(val) ||
    ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, val);
}

function validateKeyCertOption(
  val: any,
  name: string,
  allowKeyObjects: boolean,
) {
  if (!val) return; // falsy values (false, null, undefined, 0, '') are skipped
  if (isValidKeyCertValue(val)) return;
  if (ArrayIsArray(val)) {
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
  if (ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, val)) {
    return new Uint8Array(val);
  }
  if (isArrayBufferView(val)) {
    const buffer = isTypedArray(val)
      ? TypedArrayPrototypeGetBuffer(val)
      : DataViewPrototypeGetBuffer(val);
    const byteOffset = isTypedArray(val)
      ? TypedArrayPrototypeGetByteOffset(val)
      : DataViewPrototypeGetByteOffset(val);
    const byteLength = isTypedArray(val)
      ? TypedArrayPrototypeGetByteLength(val)
      : DataViewPrototypeGetByteLength(val);
    return new Uint8Array(buffer, byteOffset, byteLength);
  }
  return new TextEncoder().encode(String(val));
}

// Node accepts `cert` as <string>|<Buffer>|<Array<string|Buffer>>. Multiple PEM
// blocks can be concatenated in a single string, so flatten array forms into
// one PEM-encoded string for the rustls path. Anything that can't be coerced
// to a PEM string is skipped.
function normalizeCertPem(val: any): string | undefined {
  if (val == null) return undefined;
  if (ArrayIsArray(val)) {
    const parts: string[] = [];
    for (const item of new SafeArrayIterator(val)) {
      if (item == null) continue;
      const s = toStringOrUndefined(item);
      if (s !== undefined && s !== "") ArrayPrototypePush(parts, s);
    }
    if (parts.length === 0) return undefined;
    return ArrayPrototypeJoin(parts, "\n");
  }
  return toStringOrUndefined(val);
}

// Node accepts `key` as <string>|<Buffer>|<Array<string|Buffer|Object>> where
// the object form is `{ pem, passphrase? }`. rustls_pemfile cannot read
// encrypted PKCS#1/PKCS#8 blocks, so decrypt any passphrase-protected entries
// via `createPrivateKey` and re-export as unencrypted PKCS#8 PEM before
// concatenating.
function normalizeKeyPem(
  val: any,
  defaultPassphrase: any,
): string | undefined {
  if (val == null) return undefined;
  const items = ArrayIsArray(val) ? val : [val];
  const parts: string[] = [];
  for (const item of new SafeArrayIterator(items)) {
    if (item == null) continue;
    let pem: string | undefined;
    let passphrase = defaultPassphrase;
    if (
      typeof item === "object" && !isArrayBufferView(item) &&
      !ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, item)
    ) {
      pem = toStringOrUndefined((item as any).pem ?? (item as any).key);
      if ((item as any).passphrase != null) {
        passphrase = (item as any).passphrase;
      }
    } else {
      pem = toStringOrUndefined(item);
    }
    if (pem === undefined || pem === "") continue;
    if (passphrase != null) {
      try {
        const keyObject = createPrivateKey({
          key: pem,
          passphrase: String(passphrase),
        });
        pem = keyObject.export({ format: "pem", type: "pkcs8" }) as string;
      } catch {
        // Fall through with the original PEM; the TLS layer will surface a
        // proper error if it really is unreadable.
      }
    }
    ArrayPrototypePush(parts, pem);
  }
  if (parts.length === 0) return undefined;
  return ArrayPrototypeJoin(parts, "\n");
}

const secureContextBrand = new SafeWeakSet<object>();

class SecureContext {
  context: {
    ca?: string | string[];
    useDefaultCA?: boolean;
    cert?: string;
    key?: string;
    minVersion: string;
    maxVersion: string;
    ciphers?: string;
    passphrase?: string;
    sigalgs?: string;
    ecdhCurve?: string;
  };

  constructor(options: any = { __proto__: null }) {
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
    if (
      options.privateKeyEngine != null &&
      options.privateKeyIdentifier != null
    ) {
      throw new ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED();
    }
    if (options.ecdhCurve != null) {
      validateString(options.ecdhCurve, "options.ecdhCurve");
    }
    // Validate cert before key - Node.js processes cert first (SetCert before SetKey)
    validateKeyCertOption(options.cert, "options.cert", false);
    validateKeyCertOption(options.key, "options.key", true);
    validateKeyCertOption(options.ca, "options.ca", false);

    // Load PFX / PKCS#12 data: extract the cert + private key so they can
    // be used by the underlying TLS implementation. Any additional certs
    // present in the PFX are merged into `ca`. Caller-supplied `cert`/`key`
    // (and `ca`) take precedence, matching Node, which loads PFX first and
    // then layers explicit cert/key on top.
    //
    // Node accepts both a single <string>|<Buffer> and an
    // <Array<string|Buffer|{ buf, passphrase? }>>; an empty array (which
    // playwright passes when no PFX is configured) must be a no-op rather
    // than feeding an empty buffer to the parser.
    let pfxCert: string | undefined;
    let pfxKey: string | undefined;
    let pfxCa: string[] | undefined;
    if (options.pfx != null) {
      const pfxItems = ArrayIsArray(options.pfx) ? options.pfx : [options.pfx];
      for (const item of new SafeArrayIterator(pfxItems)) {
        if (item == null) continue;
        let buf: any = item;
        let passphrase: any = options.passphrase;
        if (
          typeof item === "object" && !isArrayBufferView(item) &&
          !ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, item) &&
          ((item as any).buf !== undefined ||
            (item as any).passphrase !== undefined)
        ) {
          buf = (item as any).buf;
          if ((item as any).passphrase != null) {
            passphrase = (item as any).passphrase;
          }
        }
        if (buf == null) continue;
        const pfxData = toUint8Array(buf);
        const pfxPassphrase = passphrase != null ? String(passphrase) : null;
        const loaded = op_node_load_pfx(pfxData, pfxPassphrase);
        if (pfxCert === undefined) {
          pfxCert = loaded.cert;
          pfxKey = loaded.key;
        }
        if (loaded.ca?.length) {
          if (pfxCa === undefined) pfxCa = [];
          ArrayPrototypePush(pfxCa, ...new SafeArrayIterator(loaded.ca));
        }
      }
    }

    // Validate CRL data.
    if (options.crl != null) {
      const crls = ArrayIsArray(options.crl) ? options.crl : [options.crl];
      for (const crl of new SafeArrayIterator(crls)) {
        op_node_validate_crl(toUint8Array(crl));
      }
    }

    // Mirror OpenSSL's SECLEVEL key-size enforcement. The cipher list is
    // applied before the key is loaded (matching Node.js' order in
    // `SecureContext::Init`), so `DEFAULT:@SECLEVEL=0` retains compatibility
    // with weak keys while `DEFAULT` rejects them.
    validateKeyStrength(options);

    const { minVersion, maxVersion } = getProtocolRange(options);

    const effectiveCa = options.ca != null ? options.ca : pfxCa;
    const useDefaultCA = !effectiveCa;
    this.context = {
      ca: useDefaultCA ? undefined : normalizeCertValue(effectiveCa),
      useDefaultCA,
      cert: normalizeCertPem(options.cert) ?? pfxCert,
      key: normalizeKeyPem(options.key, options.passphrase) ?? pfxKey,
      minVersion,
      maxVersion,
      ciphers: options.ciphers,
      passphrase: options.passphrase,
      sigalgs: options.sigalgs,
      ecdhCurve: options.ecdhCurve,
    };
    secureContextBrand.add(this.context);
    ObjectDefineProperty(this.context, "_external", {
      __proto__: null,
      configurable: true,
      enumerable: false,
      get(this: object) {
        if (!WeakSetPrototypeHas(secureContextBrand, this)) {
          throw new TypeError("Illegal invocation");
        }
        return this;
      },
    });
    (this.context as any).setOptions = function setOptions(
      this: object,
      _options?: number,
    ) {
      if (!WeakSetPrototypeHas(secureContextBrand, this)) {
        throw new TypeError("Illegal invocation");
      }
    };
    (this.context as any).addCACert = function addCACert(
      this: any,
      cert: any,
    ) {
      if (!WeakSetPrototypeHas(secureContextBrand, this)) {
        throw new TypeError("Illegal invocation");
      }
      validateKeyCertOption(cert, "cert", false);
      const normalized = toStringOrUndefined(cert);
      if (normalized === undefined) {
        return;
      }
      if (this.ca === undefined) {
        this.ca = normalized;
      } else if (ArrayIsArray(this.ca)) {
        ArrayPrototypePush(this.ca, normalized);
      } else {
        this.ca = [this.ca, normalized];
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

function createSecureContext(options: any = { __proto__: null }) {
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

    StringPrototypeReplace(
      info,
      new SafeRegExp("([^\\n:]*):([^\\n]*)(?:\\n|$)", "g"),
      (_all: string, key: string, value: string) => {
        const normalized = StringPrototypeCharCodeAt(value, 0) === 0x22
          ? JSONParse(value)
          : value;
        if (ObjectHasOwn(c.infoAccess, key)) {
          ArrayPrototypePush(c.infoAccess[key], normalized);
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
