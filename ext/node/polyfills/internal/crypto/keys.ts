// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

(function () {
const { core, primordials } = globalThis.__bootstrap;

const {
  ArrayPrototypeIncludes,
  ObjectDefineProperties,
  SymbolToStringTag,
} = primordials;

const {
  op_node_create_ec_jwk,
  op_node_create_ed_raw,
  op_node_create_private_key,
  op_node_create_public_key,
  op_node_create_rsa_jwk,
  op_node_create_secret_key,
  op_node_derive_public_key_from_private_key,
  op_node_export_private_key_der,
  op_node_export_private_key_jwk,
  op_node_export_private_key_pem,
  op_node_export_public_key_der,
  op_node_export_public_key_jwk,
  op_node_export_public_key_pem,
  op_node_export_secret_key,
  op_node_export_secret_key_b64url,
  op_node_get_asymmetric_key_details,
  op_node_get_asymmetric_key_type,
  op_node_get_symmetric_key_size,
  op_node_key_equals,
  op_node_key_type,
} = core.ops;

const {
  cryptoKeyExportNodeKeyMaterial,
  importCryptoKeySync,
} = core.loadExtScript("ext:deno_crypto/00_crypto.js");

const { kHandle } = core.loadExtScript(
  "ext:deno_node/internal/crypto/constants.ts",
);

// Lazy import of cipher.ts to break circular dependency
const lazyCipher = () =>
  core.loadExtScript("ext:deno_node/internal/crypto/cipher.ts");

const {
  ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS,
  ERR_CRYPTO_INVALID_JWK,
  ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { notImplemented } = core.loadExtScript("ext:deno_node/_utils.ts");
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const {
  isAnyArrayBuffer,
  isArrayBufferView,
} = core.loadExtScript("ext:deno_node/internal/util/types.ts");
const { hideStackFrames } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const {
  isCryptoKey,
  isKeyObject,
  kKeyType,
} = core.loadExtScript("ext:deno_node/internal/crypto/_keys.ts");
const {
  validateObject,
  validateOneOf,
  validateString,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");

const getArrayBufferOrView = hideStackFrames(
  (
    buffer: ArrayBufferView | ArrayBuffer | string | Buffer,
    name: string,
    encoding?: any,
  ):
    | ArrayBuffer
    | SharedArrayBuffer
    | Buffer
    | DataView
    | BigInt64Array
    | BigUint64Array
    | Float32Array
    | Float64Array
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint8ClampedArray
    | Uint16Array
    | Uint32Array => {
    if (isAnyArrayBuffer(buffer)) {
      return new Uint8Array(buffer);
    }
    if (typeof buffer === "string") {
      if (encoding === "buffer") {
        encoding = "utf8";
      }
      return Buffer.from(buffer, encoding);
    }
    if (buffer instanceof DataView) {
      return new Uint8Array(
        buffer.buffer,
        buffer.byteOffset,
        buffer.byteLength,
      );
    }
    if (!isArrayBufferView(buffer)) {
      throw new ERR_INVALID_ARG_TYPE(
        name,
        [
          "string",
          "ArrayBuffer",
          "Buffer",
          "TypedArray",
          "DataView",
        ],
        buffer,
      );
    }
    return buffer;
  },
);

const kConsumePublic = 0;
const kConsumePrivate = 1;
const kCreatePublic = 2;
const kCreatePrivate = 3;

class KeyObject {
  [kKeyType]: any;
  [kHandle]: any;

  constructor(type: any, handle: any) {
    if (type !== "secret" && type !== "public" && type !== "private") {
      throw new ERR_INVALID_ARG_VALUE("type", type);
    }

    if (typeof handle !== "object") {
      throw new ERR_INVALID_ARG_TYPE("handle", "object", handle);
    }

    this[kKeyType] = type;
    this[kHandle] = handle;
  }

  get type(): any {
    return this[kKeyType];
  }

  static from(key: CryptoKey): KeyObject {
    if (!isCryptoKey(key)) {
      throw new ERR_INVALID_ARG_TYPE("key", "CryptoKey", key);
    }
    const { type, data } = cryptoKeyExportNodeKeyMaterial(key);
    if (type === "secret") {
      const handle = op_node_create_secret_key(data);
      return new SecretKeyObject(handle);
    } else if (type === "public") {
      const handle = op_node_create_public_key(data, "der", "spki");
      return new PublicKeyObject(handle);
    } else {
      const handle = op_node_create_private_key(
        data,
        "der",
        "pkcs8",
        undefined,
      );
      return new PrivateKeyObject(handle);
    }
  }

  equals(otherKeyObject: KeyObject): boolean {
    if (!isKeyObject(otherKeyObject)) {
      throw new ERR_INVALID_ARG_TYPE(
        "otherKeyObject",
        "KeyObject",
        otherKeyObject,
      );
    }

    return op_node_key_equals(this[kHandle], otherKeyObject[kHandle]);
  }

  export(_options?: unknown): string | Buffer | JsonWebKey {
    notImplemented("crypto.KeyObject.prototype.export");
  }
}

ObjectDefineProperties(KeyObject.prototype, {
  [SymbolToStringTag]: {
    // @ts-expect-error __proto__ is magic
    __proto__: null,
    configurable: true,
    value: "KeyObject",
  },
});

function getKeyObjectHandle(key: KeyObject, ctx: number) {
  if (ctx === kCreatePrivate) {
    throw new ERR_INVALID_ARG_TYPE(
      "key",
      ["string", "ArrayBuffer", "Buffer", "TypedArray", "DataView"],
      key,
    );
  }

  if (key.type !== "private") {
    if (ctx === kConsumePrivate || ctx === kCreatePublic) {
      throw new ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE(key.type, "private");
    }
    if (key.type !== "public") {
      throw new ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE(
        key.type,
        "private or public",
      );
    }
  }

  return key[kHandle];
}

function getKeyObjectHandleFromJwk(key, ctx) {
  validateObject(key, "key");
  validateOneOf(
    key.kty,
    "key.kty",
    ["RSA", "EC", "OKP"],
  );
  const isPublic = ctx === kConsumePublic || ctx === kCreatePublic;

  if (key.kty === "OKP") {
    validateString(key.crv, "key.crv");
    validateOneOf(
      key.crv,
      "key.crv",
      ["Ed25519", "Ed448", "X25519", "X448"],
    );
    validateString(key.x, "key.x");

    if (!isPublic) {
      validateString(key.d, "key.d");
    }

    let keyData;
    if (isPublic) {
      keyData = Buffer.from(key.x, "base64");
    } else {
      keyData = Buffer.from(key.d, "base64");
    }

    switch (key.crv) {
      case "Ed25519":
      case "X25519":
        if (keyData.byteLength !== 32) {
          throw new ERR_CRYPTO_INVALID_JWK();
        }
        break;
      case "Ed448":
        if (keyData.byteLength !== 57) {
          throw new ERR_CRYPTO_INVALID_JWK();
        }
        break;
      case "X448":
        if (keyData.byteLength !== 56) {
          throw new ERR_CRYPTO_INVALID_JWK();
        }
        break;
    }

    return op_node_create_ed_raw(key.crv, keyData, isPublic);
  }

  if (key.kty === "EC") {
    validateString(key.crv, "key.crv");
    validateString(key.x, "key.x");
    validateString(key.y, "key.y");

    if (!isPublic) {
      validateString(key.d, "key.d");
    }

    return op_node_create_ec_jwk(key, isPublic);
  }

  // RSA
  validateString(key.n, "key.n");
  validateString(key.e, "key.e");

  const jwk = {
    kty: key.kty,
    n: key.n,
    e: key.e,
  };

  if (!isPublic) {
    validateString(key.d, "key.d");
    validateString(key.p, "key.p");
    validateString(key.q, "key.q");
    validateString(key.dp, "key.dp");
    validateString(key.dq, "key.dq");
    validateString(key.qi, "key.qi");
    jwk.d = key.d;
    jwk.p = key.p;
    jwk.q = key.q;
    jwk.dp = key.dp;
    jwk.dq = key.dq;
    jwk.qi = key.qi;
  }

  return op_node_create_rsa_jwk(jwk, isPublic);
}

function isStringOrBuffer(val: unknown): boolean {
  return typeof val === "string" ||
    isArrayBufferView(val) ||
    isAnyArrayBuffer(val) ||
    Buffer.isBuffer(val);
}

function prepareAsymmetricKey(
  key: any,
  ctx: number,
): any {
  if (isKeyObject(key)) {
    return {
      // @ts-ignore __proto__ is magic
      __proto__: null,
      handle: getKeyObjectHandle(key, ctx),
    };
  } else if (isCryptoKey(key)) {
    return {
      // @ts-ignore __proto__ is magic
      __proto__: null,
      handle: KeyObject.from(key)[kHandle],
    };
  } else if (lazyCipher().isStringOrBuffer(key)) {
    return {
      // @ts-ignore __proto__ is magic
      __proto__: null,
      format: "pem",
      data: getArrayBufferOrView(key, "key"),
    };
  } else if (typeof key === "object") {
    const { key: data, format } = key;
    if (isKeyObject(data)) {
      return {
        // @ts-ignore __proto__ is magic
        __proto__: null,
        handle: getKeyObjectHandle(data, ctx),
      };
    } else if (isCryptoKey(data)) {
      return {
        // @ts-ignore __proto__ is magic
        __proto__: null,
        handle: KeyObject.from(data)[kHandle],
      };
    } else if (format === "jwk") {
      if (typeof data !== "object" || data === null) {
        throw new ERR_INVALID_ARG_TYPE("key.key", "object", data);
      }
      return {
        // @ts-ignore __proto__ is magic
        __proto__: null,
        handle: getKeyObjectHandleFromJwk(data, ctx),
        format,
      };
    }
    if (!lazyCipher().isStringOrBuffer(data)) {
      throw new ERR_INVALID_ARG_TYPE(
        "key.key",
        getKeyTypes(ctx !== kCreatePrivate),
        data,
      );
    }

    const isPublic = (ctx === kConsumePrivate || ctx === kCreatePrivate)
      ? false
      : undefined;
    return {
      data: getArrayBufferOrView(
        data,
        "key",
        key.encoding,
      ),
      ...parseKeyEncoding(key, undefined, isPublic),
    };
  }
  throw new ERR_INVALID_ARG_TYPE(
    "key",
    getKeyTypes(ctx !== kCreatePrivate),
    key,
  );
}

function parseKeyEncoding(
  enc: any,
  keyType: string | undefined,
  isPublic: boolean | undefined,
  objName?: string,
): any {
  if (enc === null || typeof enc !== "object") {
    throw new ERR_INVALID_ARG_TYPE("options", "object", enc);
  }

  const isInput = keyType === undefined;

  const {
    format,
    type,
  } = parseKeyFormatAndType(enc, keyType, isPublic, objName);

  let cipher, passphrase, encoding;
  if (isPublic !== true) {
    ({ cipher, passphrase, encoding } = enc);

    if (!isInput) {
      if (cipher != null) {
        if (typeof cipher !== "string") {
          throw new ERR_INVALID_ARG_VALUE(option("cipher", objName), cipher);
        }
        if (
          format === "der" &&
          (type === "pkcs1" || type === "sec1")
        ) {
          throw new ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS(
            type,
            "does not support encryption",
          );
        }
      } else if (passphrase !== undefined) {
        throw new ERR_INVALID_ARG_VALUE(option("cipher", objName), cipher);
      }
    }

    if (
      (isInput && passphrase !== undefined &&
        !isStringOrBuffer(passphrase)) ||
      (!isInput && cipher != null && !isStringOrBuffer(passphrase))
    ) {
      throw new ERR_INVALID_ARG_VALUE(
        option("passphrase", objName),
        passphrase,
      );
    }
  }

  if (passphrase !== undefined) {
    passphrase = getArrayBufferOrView(passphrase, "key.passphrase", encoding);
  }

  return {
    // @ts-ignore __proto__ is magic
    __proto__: null,
    format,
    type,
    cipher,
    passphrase,
  };
}

function option(name: string, objName?: string) {
  return objName === undefined
    ? `options.${name}`
    : `options.${objName}.${name}`;
}

function parseKeyFormatAndType(
  enc: { format?: string; type?: string },
  keyType: string | undefined,
  isPublic: boolean | undefined,
  objName?: string,
): any {
  const { format: formatStr, type: typeStr } = enc;

  const isInput = keyType === undefined;
  const format = parseKeyFormat(
    formatStr,
    isInput ? "pem" : undefined,
    option("format", objName),
  );

  const type = parseKeyType(
    typeStr,
    !isInput || format === "der",
    keyType,
    isPublic,
    option("type", objName),
  );

  return {
    // @ts-ignore __proto__ is magic
    __proto__: null,
    format,
    type,
  };
}

function parseKeyFormat(
  formatStr: string | undefined,
  defaultFormat: any,
  optionName: string,
): any {
  if (formatStr === undefined && defaultFormat !== undefined) {
    return defaultFormat;
  } else if (formatStr === "pem") {
    return "pem";
  } else if (formatStr === "der") {
    return "der";
  }
  throw new ERR_INVALID_ARG_VALUE(optionName, formatStr);
}

function parseKeyType(
  typeStr: string | undefined,
  required: boolean,
  keyType: string | undefined,
  isPublic: boolean | undefined,
  optionName: string,
): any {
  if (typeStr === undefined && !required) {
    return undefined;
  } else if (typeStr === "pkcs1") {
    if (keyType !== undefined && keyType !== "rsa") {
      throw new ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS(
        typeStr,
        "can only be used for RSA keys",
      );
    }
    return "pkcs1";
  } else if (typeStr === "spki" && isPublic !== false) {
    return "spki";
  } else if (typeStr === "pkcs8" && isPublic !== true) {
    return "pkcs8";
  } else if (typeStr === "sec1" && isPublic !== true) {
    if (keyType !== undefined && keyType !== "ec") {
      throw new ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS(
        typeStr,
        "can only be used for EC keys",
      );
    }
    return "sec1";
  }
  throw new ERR_INVALID_ARG_VALUE(optionName, typeStr);
}

function parsePublicKeyEncoding(
  enc: any,
  keyType: string | undefined,
  objName?: string,
) {
  return parseKeyEncoding(enc, keyType, keyType ? true : undefined, objName);
}

function parsePrivateKeyEncoding(
  enc: any,
  keyType: string | undefined,
  objName?: string,
) {
  return parseKeyEncoding(enc, keyType, false, objName);
}

function decorateOsslDecoderError(err: unknown): unknown {
  const e = err as any;
  if (
    e && typeof e.message === "string" &&
    e.message.startsWith("error:1E08010C:DECODER routines::unsupported")
  ) {
    if (e.library === undefined) e.library = "DECODER routines";
  }
  return err;
}

function createPrivateKey(
  key: any,
): PrivateKeyObject {
  const res = prepareAsymmetricKey(key, kCreatePrivate);
  if ("handle" in res) {
    const type = op_node_key_type(res.handle);
    if (type === "private") {
      return new PrivateKeyObject(res.handle);
    } else {
      throw new TypeError(`Can not create private key from ${type} key`);
    }
  } else {
    let handle;
    try {
      handle = op_node_create_private_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
    } catch (err) {
      throw decorateOsslDecoderError(err);
    }
    return new PrivateKeyObject(handle);
  }
}

function createPublicKey(
  key: any,
): PublicKeyObject {
  const res = prepareAsymmetricKey(
    key,
    kCreatePublic,
  );
  if ("handle" in res) {
    const type = op_node_key_type(res.handle);
    if (type === "private") {
      const handle = op_node_derive_public_key_from_private_key(res.handle);
      return new PublicKeyObject(handle);
    } else if (type === "public") {
      return new PublicKeyObject(res.handle);
    } else {
      throw new TypeError(`Can not create private key from ${type} key`);
    }
  } else {
    let handle;
    try {
      handle = op_node_create_public_key(
        res.data,
        res.format,
        res.type ?? "",
        res.passphrase,
      );
    } catch (err) {
      throw decorateOsslDecoderError(err);
    }
    return new PublicKeyObject(handle);
  }
}

function getKeyTypes(allowKeyObject: boolean, bufferOnly = false) {
  const types = [
    "ArrayBuffer",
    "Buffer",
    "TypedArray",
    "DataView",
    "string",
    "KeyObject",
    "CryptoKey",
  ];
  if (bufferOnly) {
    return types.slice(0, 4);
  } else if (!allowKeyObject) {
    return types.slice(0, 5);
  }
  return types;
}

function prepareSecretKey(
  key: string | ArrayBufferView | ArrayBuffer | KeyObject | CryptoKey,
  encoding: string | undefined,
  bufferOnly = false,
): Buffer | ArrayBuffer | ArrayBufferView | any {
  if (!bufferOnly) {
    if (isKeyObject(key)) {
      if (key.type !== "secret") {
        throw new ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE(key.type, "secret");
      }
      return key[kHandle];
    } else if (isCryptoKey(key)) {
      if (key.type !== "secret") {
        throw new ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE(key.type, "secret");
      }
      return KeyObject.from(key)[kHandle];
    }
  }
  if (
    typeof key !== "string" &&
    !isArrayBufferView(key) &&
    !isAnyArrayBuffer(key)
  ) {
    throw new ERR_INVALID_ARG_TYPE(
      "key",
      getKeyTypes(!bufferOnly, bufferOnly),
      key,
    );
  }

  return getArrayBufferOrView(key, "key", encoding);
}

class SecretKeyObject extends KeyObject {
  constructor(handle: any) {
    super("secret", handle);
  }

  get symmetricKeySize() {
    return op_node_get_symmetric_key_size(this[kHandle]);
  }

  get asymmetricKeyType() {
    return undefined;
  }

  toCryptoKey(
    algorithm: string | object,
    extractable: boolean,
    usages: string[],
  ): CryptoKey {
    const algName = typeof algorithm === "string"
      ? algorithm
      : (algorithm as { name: string }).name;

    const rawData = new Uint8Array(op_node_export_secret_key(this[kHandle]));

    if (rawData.byteLength === 0) {
      throw new DOMException(
        "Zero-length key is not supported",
        "DataError",
      );
    }

    if (algName === "PBKDF2") {
      if (extractable) {
        throw new DOMException(
          "PBKDF2 keys are not extractable",
          "SyntaxError",
        );
      }
      if (
        usages.length > 0 &&
        usages.some((u: string) =>
          !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u)
        )
      ) {
        throw new DOMException(
          "Unsupported key usage for a PBKDF2 key",
          "SyntaxError",
        );
      }
    } else if (algName === "HKDF") {
      if (extractable) {
        throw new DOMException(
          "HKDF keys are not extractable",
          "SyntaxError",
        );
      }
      if (
        usages.length > 0 &&
        usages.some((u: string) =>
          !ArrayPrototypeIncludes(["deriveKey", "deriveBits"], u)
        )
      ) {
        throw new DOMException(
          "Unsupported key usage for an HKDF key",
          "SyntaxError",
        );
      }
    } else if (algName === "HMAC") {
      if (usages.length === 0) {
        throw new DOMException(
          "Usages cannot be empty when importing a secret key.",
          "SyntaxError",
        );
      }
      const alg = algorithm as { length?: number };
      if (alg.length !== undefined && alg.length === 0) {
        throw new DOMException(
          "HmacImportParams.length cannot be 0",
          "DataError",
        );
      }
    } else if (algName === "KMAC128" || algName === "KMAC256") {
      if (usages.length === 0) {
        throw new DOMException(
          "Usages cannot be empty when importing a secret key.",
          "SyntaxError",
        );
      }
      const alg = algorithm as { length?: number };
      if (alg.length !== undefined && alg.length === 0) {
        throw new DOMException(
          "KmacImportParams.length cannot be 0",
          "DataError",
        );
      }
    } else {
      if (usages.length === 0) {
        throw new DOMException(
          "Usages cannot be empty when importing a secret key.",
          "SyntaxError",
        );
      }
    }

    return importCryptoKeySync("raw", rawData, algorithm, extractable, usages);
  }

  export(options?: { format?: "buffer" | "jwk" }): Buffer | JsonWebKey {
    let format: "buffer" | "jwk" = "buffer";
    if (options !== undefined) {
      validateObject(options, "options");
      validateOneOf(
        options.format,
        "options.format",
        [undefined, "buffer", "jwk"],
      );
      format = options.format ?? "buffer";
    }
    switch (format) {
      case "buffer":
        return Buffer.from(op_node_export_secret_key(this[kHandle]));
      case "jwk":
        return {
          kty: "oct",
          k: op_node_export_secret_key_b64url(this[kHandle]),
        };
    }
  }
}

class AsymmetricKeyObject extends KeyObject {
  constructor(type: any, handle: any) {
    super(type, handle);
  }

  get asymmetricKeyType() {
    return op_node_get_asymmetric_key_type(this[kHandle]);
  }

  get asymmetricKeyDetails() {
    return { ...op_node_get_asymmetric_key_details(this[kHandle]) };
  }
}

class PrivateKeyObject extends AsymmetricKeyObject {
  constructor(handle: any) {
    super("private", handle);
  }

  toCryptoKey(
    algorithm: string | object,
    extractable: boolean,
    usages: string[],
  ): CryptoKey {
    const algName = typeof algorithm === "string"
      ? algorithm
      : (algorithm as { name: string }).name;

    _validateAsymmetricKeyAlgorithm(this, algName);
    if (typeof algorithm === "object") {
      _validateEcNamedCurve(this, algorithm);
    }

    if (usages.length === 0) {
      throw new DOMException(
        "Usages cannot be empty when importing a private key.",
        "SyntaxError",
      );
    }

    const pkcs8Data = Buffer.from(
      op_node_export_private_key_der(this[kHandle], "pkcs8", null, null),
    );
    return importCryptoKeySync(
      "pkcs8",
      pkcs8Data,
      algorithm,
      extractable,
      usages,
    );
  }

  export(options: any) {
    if (options && options.format === "jwk") {
      if (
        (options as { cipher?: unknown }).cipher !== undefined ||
        (options as { passphrase?: unknown }).passphrase !== undefined
      ) {
        throw new ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS(
          "jwk",
          "does not support encryption",
        );
      }
      return { ...op_node_export_private_key_jwk(this[kHandle]) };
    }
    const {
      format,
      type,
      cipher,
      passphrase,
    } = parsePrivateKeyEncoding(options, this.asymmetricKeyType);

    if (format === "pem") {
      return op_node_export_private_key_pem(
        this[kHandle],
        type,
        cipher ?? null,
        passphrase != null ? passphrase.toString() : null,
      );
    } else {
      return Buffer.from(
        op_node_export_private_key_der(
          this[kHandle],
          type,
          cipher ?? null,
          passphrase != null ? passphrase.toString() : null,
        ),
      );
    }
  }
}

class PublicKeyObject extends AsymmetricKeyObject {
  constructor(handle: any) {
    super("public", handle);
  }

  toCryptoKey(
    algorithm: string | object,
    extractable: boolean,
    usages: string[],
  ): CryptoKey {
    const algName = typeof algorithm === "string"
      ? algorithm
      : (algorithm as { name: string }).name;

    _validateAsymmetricKeyAlgorithm(this, algName);
    if (typeof algorithm === "object") {
      _validateEcNamedCurve(this, algorithm);
    }

    const spkiData = Buffer.from(
      op_node_export_public_key_der(this[kHandle], "spki"),
    );
    return importCryptoKeySync(
      "spki",
      spkiData,
      algorithm,
      extractable,
      usages,
    );
  }

  export(options: any) {
    if (options && options.format === "jwk") {
      return { ...op_node_export_public_key_jwk(this[kHandle]) };
    }

    const {
      format,
      type,
    } = parsePublicKeyEncoding(options, this.asymmetricKeyType);

    if (format === "pem") {
      return op_node_export_public_key_pem(this[kHandle], type);
    } else {
      return Buffer.from(op_node_export_public_key_der(this[kHandle], type));
    }
  }
}

function _validateAsymmetricKeyAlgorithm(
  keyObject: AsymmetricKeyObject,
  algName: string,
) {
  const keyType = keyObject.asymmetricKeyType;

  if (keyType === "ed25519" || keyType === "x25519") {
    const expectedAlg = keyType === "ed25519" ? "Ed25519" : "X25519";
    if (algName !== expectedAlg) {
      throw new DOMException("Invalid key type", "DataError");
    }
  } else if (keyType === "ed448" || keyType === "x448") {
    const expectedAlg = keyType === "ed448" ? "Ed448" : "X448";
    if (algName !== expectedAlg) {
      throw new DOMException("Invalid key type", "DataError");
    }
  }
}

function _validateEcNamedCurve(
  keyObject: AsymmetricKeyObject,
  algorithm: object,
) {
  const details = keyObject.asymmetricKeyDetails;
  const alg = algorithm as { namedCurve?: string };
  if (alg.namedCurve && details?.namedCurve) {
    const curveMap: Record<string, string> = {
      "prime256v1": "P-256",
      "secp384r1": "P-384",
      "secp521r1": "P-521",
      "P-256": "P-256",
      "P-384": "P-384",
      "P-521": "P-521",
    };
    const keyCurve = curveMap[details.namedCurve] || details.namedCurve;
    if (keyCurve !== alg.namedCurve) {
      throw new DOMException("Named curve mismatch", "DataError");
    }
  }
}

function createSecretKey(
  key: string | ArrayBufferView | ArrayBuffer | KeyObject | CryptoKey,
  encoding?: string,
): KeyObject {
  if (isCryptoKey(key)) {
    if (key.type !== "secret") {
      throw new ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE(key.type, "secret");
    }
    return KeyObject.from(key);
  }
  const preparedKey = prepareSecretKey(key, encoding, true);
  if (isArrayBufferView(preparedKey) || isAnyArrayBuffer(preparedKey)) {
    const handle = op_node_create_secret_key(preparedKey);
    return new SecretKeyObject(handle);
  } else {
    const type = op_node_key_type(preparedKey);
    if (type === "secret") {
      return new SecretKeyObject(preparedKey);
    } else {
      throw new TypeError(`can not create secret key from ${type} key`);
    }
  }
}

return {
  getArrayBufferOrView,
  KeyObject,
  kConsumePublic,
  kConsumePrivate,
  kCreatePublic,
  kCreatePrivate,
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  prepareSecretKey,
  prepareAsymmetricKey,
  getKeyObjectHandle,
  SecretKeyObject,
  PrivateKeyObject,
  PublicKeyObject,
  default: {
    createPrivateKey,
    createPublicKey,
    createSecretKey,
    KeyObject,
    prepareSecretKey,
    SecretKeyObject,
    PrivateKeyObject,
    PublicKeyObject,
  },
};
})();
