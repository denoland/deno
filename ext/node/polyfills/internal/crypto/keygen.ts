// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file no-explicit-any prefer-primordials

(function () {
const { core } = globalThis.__bootstrap;
const {
  PrivateKeyObject,
  PublicKeyObject,
  SecretKeyObject,
} = core.loadExtScript("ext:deno_node/internal/crypto/keys.ts");
const { kAesKeyLengths } = core.loadExtScript(
  "ext:deno_node/internal/crypto/util.ts",
);
const {
  ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS,
  ERR_CRYPTO_INVALID_DIGEST,
  ERR_CRYPTO_UNKNOWN_CIPHER,
  ERR_CRYPTO_UNKNOWN_DH_GROUP,
  ERR_INCOMPATIBLE_OPTION_PAIR,
  ERR_INVALID_ARG_VALUE,
  ERR_MISSING_OPTION,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { getCiphers } = core.loadExtScript(
  "ext:deno_node/internal/crypto/util.ts",
);
const { getHashes } = core.loadExtScript(
  "ext:deno_node/internal/crypto/hash.ts",
);
const {
  validateBuffer,
  validateFunction,
  validateInt32,
  validateInteger,
  validateObject,
  validateOneOf,
  validateString,
  validateUint32,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");

const lazyProcess = core.createLazyLoader("node:process");

const { promisify } = core.loadExtScript("ext:deno_node/util.ts");

const {
  op_node_generate_dh_group_key,
  op_node_generate_dh_group_key_async,
  op_node_generate_dh_key,
  op_node_generate_dh_key_async,
  op_node_generate_dsa_key,
  op_node_generate_dsa_key_async,
  op_node_generate_ec_key,
  op_node_generate_ec_key_async,
  op_node_generate_ed25519_key,
  op_node_generate_ed25519_key_async,
  op_node_generate_ed448_key,
  op_node_generate_ed448_key_async,
  op_node_generate_rsa_key,
  op_node_generate_rsa_key_async,
  op_node_generate_rsa_pss_key,
  op_node_generate_rsa_pss_key_async,
  op_node_generate_secret_key,
  op_node_generate_secret_key_async,
  op_node_generate_x25519_key,
  op_node_generate_x25519_key_async,
  op_node_generate_x448_key,
  op_node_generate_x448_key_async,
  op_node_get_private_key_from_pair,
  op_node_get_public_key_from_pair,
} = core.ops;

function validateGenerateKey(
  type: "hmac" | "aes",
  options: { length: number },
) {
  validateString(type, "type");
  validateObject(options, "options");
  const { length } = options;
  switch (type) {
    case "hmac":
      validateInteger(length, "options.length", 8, 2 ** 31 - 1);
      break;
    case "aes":
      validateOneOf(length, "options.length", kAesKeyLengths);
      break;
    default:
      throw new ERR_INVALID_ARG_VALUE(
        "type",
        type,
        "must be a supported key type",
      );
  }
}

function generateKeySync(
  type: "hmac" | "aes",
  options: {
    length: number;
  },
): KeyObject {
  validateGenerateKey(type, options);
  const { length } = options;

  const len = Math.floor(length / 8);

  const handle = op_node_generate_secret_key(len);

  return new SecretKeyObject(handle);
}

function generateKey(
  type: "hmac" | "aes",
  options: {
    length: number;
  },
  callback: (err: Error | null, key: KeyObject) => void,
) {
  validateGenerateKey(type, options);
  validateFunction(callback, "callback");
  const { length } = options;

  const len = Math.floor(length / 8);

  op_node_generate_secret_key_async(len).then((handle) => {
    callback(null, new SecretKeyObject(handle));
  });
}

function generateKeyPair(
  type: any,
  options: unknown,
  callback?: (
    err: Error | null,
    publicKey: any,
    privateKey: any,
  ) => void,
) {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  validateFunction(callback, "callback");

  _generateKeyPair(type, options)
    .then(
      (res) => callback!(null, res.publicKey, res.privateKey),
      (err) => callback!(err, null, null),
    );
}

function _generateKeyPair(type: string, options: unknown) {
  return createJob(kAsync, type, options).then((pair) => {
    const privateKeyHandle = op_node_get_private_key_from_pair(pair);
    const publicKeyHandle = op_node_get_public_key_from_pair(pair);

    let privateKey = new PrivateKeyObject(privateKeyHandle);
    let publicKey = new PublicKeyObject(publicKeyHandle);

    if (typeof options === "object" && options !== null) {
      const { publicKeyEncoding, privateKeyEncoding } = options as any;

      if (publicKeyEncoding) {
        publicKey = publicKey.export(publicKeyEncoding);
      }

      if (privateKeyEncoding) {
        privateKey = privateKey.export(privateKeyEncoding);
      }
    }

    return { publicKey, privateKey };
  });
}

Object.defineProperty(generateKeyPair, promisify.custom, {
  enumerable: false,
  value: _generateKeyPair,
});

function generateKeyPairSync(
  type: any,
  options: unknown,
): any {
  const pair = createJob(kSync, type, options);

  const privateKeyHandle = op_node_get_private_key_from_pair(pair);
  const publicKeyHandle = op_node_get_public_key_from_pair(pair);

  let privateKey = new PrivateKeyObject(privateKeyHandle);
  let publicKey = new PublicKeyObject(publicKeyHandle);

  if (typeof options === "object" && options !== null) {
    const { publicKeyEncoding, privateKeyEncoding } = options as any;

    if (publicKeyEncoding) {
      publicKey = publicKey.export(publicKeyEncoding);
    }

    if (privateKeyEncoding) {
      privateKey = privateKey.export(privateKeyEncoding);
    }
  }

  return { publicKey, privateKey };
}

const kSync = 0;
const kAsync = 1;

function parseKeyFormat(
  formatStr: string | undefined,
  defaultFormat: string | undefined,
  optionName: string,
): string | undefined {
  if (formatStr === undefined && defaultFormat !== undefined) {
    return defaultFormat;
  } else if (formatStr === "pem") {
    return "pem";
  } else if (formatStr === "der") {
    return "der";
  } else if (formatStr === "jwk") {
    return "jwk";
  }
  throw new ERR_INVALID_ARG_VALUE(optionName, formatStr);
}

function parseKeyType(
  typeStr: string | undefined,
  required: boolean,
  keyType: string | undefined,
  isPublic: boolean | undefined,
  optionName: string,
): string | undefined {
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

function option(name: string, objName?: string): string {
  return objName === undefined
    ? `options.${name}`
    : `options.${objName}.${name}`;
}

function isStringOrBuffer(val: unknown): boolean {
  return typeof val === "string" ||
    ArrayBuffer.isView(val) ||
    val instanceof ArrayBuffer ||
    val instanceof SharedArrayBuffer;
}

function parseKeyFormatAndType(
  enc: any,
  keyType: string | undefined,
  isPublic: boolean | undefined,
  objName?: string,
) {
  const { format: formatStr, type: typeStr } = enc;

  const isInput = keyType === undefined;
  const format = parseKeyFormat(
    formatStr,
    isInput ? "pem" : undefined,
    option("format", objName),
  );

  const isRequired = (!isInput || format === "der") && format !== "jwk";
  const type = parseKeyType(
    typeStr,
    isRequired,
    keyType,
    isPublic,
    option("type", objName),
  );
  return { format, type };
}

function parsePublicKeyEncoding(
  enc: any,
  keyType: string | undefined,
  objName: string,
) {
  validateObject(enc, "options");

  const { format, type } = parseKeyFormatAndType(
    enc,
    keyType,
    keyType ? true : undefined,
    objName,
  );

  return { format, type };
}

function parsePrivateKeyEncoding(
  enc: any,
  keyType: string | undefined,
  objName: string,
) {
  validateObject(enc, "options");

  const { format, type } = parseKeyFormatAndType(enc, keyType, false, objName);

  const { cipher, passphrase } = enc;

  if (cipher != null) {
    if (typeof cipher !== "string") {
      throw new ERR_INVALID_ARG_VALUE(option("cipher", objName), cipher);
    }
    if (!getCiphers().includes(cipher)) {
      throw new ERR_CRYPTO_UNKNOWN_CIPHER();
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

  if (cipher != null && !isStringOrBuffer(passphrase)) {
    throw new ERR_INVALID_ARG_VALUE(option("passphrase", objName), passphrase);
  }

  return { format, type, cipher, passphrase };
}

function validateKeyEncoding(keyType: string, options: any) {
  if (options == null || typeof options !== "object") return;

  const { publicKeyEncoding, privateKeyEncoding } = options;

  if (publicKeyEncoding != null) {
    if (typeof publicKeyEncoding === "object") {
      parsePublicKeyEncoding(publicKeyEncoding, keyType, "publicKeyEncoding");
    } else {
      throw new ERR_INVALID_ARG_VALUE(
        "options.publicKeyEncoding",
        publicKeyEncoding,
      );
    }
  }

  if (privateKeyEncoding != null) {
    if (typeof privateKeyEncoding === "object") {
      parsePrivateKeyEncoding(
        privateKeyEncoding,
        keyType,
        "privateKeyEncoding",
      );
    } else {
      throw new ERR_INVALID_ARG_VALUE(
        "options.privateKeyEncoding",
        privateKeyEncoding,
      );
    }
  }
}

function createJob(mode, type, options) {
  validateString(type, "type");

  validateKeyEncoding(type, options);

  if (options !== undefined) {
    validateObject(options, "options");
  }

  const process = lazyProcess().default;

  switch (type) {
    case "rsa":
    case "rsa-pss": {
      validateObject(options, "options");
      const { modulusLength } = options;
      validateUint32(modulusLength, "options.modulusLength");

      let { publicExponent } = options;
      if (publicExponent == null) {
        publicExponent = 0x10001;
      } else {
        validateUint32(publicExponent, "options.publicExponent");
      }

      if (type === "rsa") {
        if (mode === kSync) {
          return op_node_generate_rsa_key(
            modulusLength,
            publicExponent,
          );
        } else {
          return op_node_generate_rsa_key_async(
            modulusLength,
            publicExponent,
          );
        }
      }

      const {
        hash,
        mgf1Hash,
        hashAlgorithm,
        mgf1HashAlgorithm,
        saltLength,
      } = options;

      if (saltLength !== undefined) {
        validateInt32(saltLength, "options.saltLength", 0);
      }
      if (hashAlgorithm !== undefined) {
        validateString(hashAlgorithm, "options.hashAlgorithm");
        if (!getHashes().includes(hashAlgorithm)) {
          throw new ERR_CRYPTO_INVALID_DIGEST(hashAlgorithm);
        }
      }
      if (mgf1HashAlgorithm !== undefined) {
        validateString(mgf1HashAlgorithm, "options.mgf1HashAlgorithm");
        if (!getHashes().includes(mgf1HashAlgorithm)) {
          throw new ERR_CRYPTO_INVALID_DIGEST(mgf1HashAlgorithm, "MGF1");
        }
      }
      if (hash !== undefined) {
        process.emitWarning(
          '"options.hash" is deprecated, ' +
            'use "options.hashAlgorithm" instead.',
          "DeprecationWarning",
          "DEP0154",
        );
        validateString(hash, "options.hash");
        if (hashAlgorithm && hash !== hashAlgorithm) {
          throw new ERR_INVALID_ARG_VALUE("options.hash", hash);
        }
      }
      if (mgf1Hash !== undefined) {
        process.emitWarning(
          '"options.mgf1Hash" is deprecated, ' +
            'use "options.mgf1HashAlgorithm" instead.',
          "DeprecationWarning",
          "DEP0154",
        );
        validateString(mgf1Hash, "options.mgf1Hash");
        if (mgf1HashAlgorithm && mgf1Hash !== mgf1HashAlgorithm) {
          throw new ERR_INVALID_ARG_VALUE("options.mgf1Hash", mgf1Hash);
        }
      }

      if (mode === kSync) {
        return op_node_generate_rsa_pss_key(
          modulusLength,
          publicExponent,
          hashAlgorithm ?? hash,
          mgf1HashAlgorithm ?? mgf1Hash,
          saltLength,
        );
      } else {
        return op_node_generate_rsa_pss_key_async(
          modulusLength,
          publicExponent,
          hashAlgorithm ?? hash,
          mgf1HashAlgorithm ?? mgf1Hash,
          saltLength,
        );
      }
    }
    case "dsa": {
      validateObject(options, "options");
      const { modulusLength } = options;
      validateUint32(modulusLength, "options.modulusLength");

      let { divisorLength } = options;
      if (divisorLength == null) {
        // Match OpenSSL defaults based on modulus length (FIPS 186-4)
        divisorLength = modulusLength <= 1024 ? 160 : 256;
      } else {
        validateInt32(divisorLength, "options.divisorLength", 0);
      }

      if (mode === kSync) {
        return op_node_generate_dsa_key(modulusLength, divisorLength);
      } else {
        return op_node_generate_dsa_key_async(
          modulusLength,
          divisorLength,
        );
      }
    }
    case "ec": {
      validateObject(options, "options");
      const { namedCurve } = options;
      validateString(namedCurve, "options.namedCurve");
      const { paramEncoding } = options;
      if (paramEncoding == null || paramEncoding === "named") {
        // pass.
      } else if (paramEncoding === "explicit") {
        process.emitWarning(
          'paramEncoding: "explicit" is not supported; ' +
            "the generated key will use named-curve encoding instead.",
          "Warning",
        );
      } else {
        throw new ERR_INVALID_ARG_VALUE("options.paramEncoding", paramEncoding);
      }

      if (mode === kSync) {
        return op_node_generate_ec_key(namedCurve);
      } else {
        return op_node_generate_ec_key_async(namedCurve);
      }
    }
    case "ed25519": {
      if (mode === kSync) {
        return op_node_generate_ed25519_key();
      }
      return op_node_generate_ed25519_key_async();
    }
    case "x25519": {
      if (mode === kSync) {
        return op_node_generate_x25519_key();
      }
      return op_node_generate_x25519_key_async();
    }
    case "ed448": {
      if (mode === kSync) {
        return op_node_generate_ed448_key();
      }
      return op_node_generate_ed448_key_async();
    }
    case "x448": {
      if (mode === kSync) {
        return op_node_generate_x448_key();
      }
      return op_node_generate_x448_key_async();
    }
    case "dh": {
      validateObject(options, "options");
      const { group, primeLength, prime, generator } = options;
      if (group != null) {
        if (prime != null) {
          throw new ERR_INCOMPATIBLE_OPTION_PAIR("group", "prime");
        }
        if (primeLength != null) {
          throw new ERR_INCOMPATIBLE_OPTION_PAIR("group", "primeLength");
        }
        if (generator != null) {
          throw new ERR_INCOMPATIBLE_OPTION_PAIR("group", "generator");
        }

        validateString(group, "options.group");

        if (
          group !== "modp5" && group !== "modp14" && group !== "modp15" &&
          group !== "modp16" && group !== "modp17" && group !== "modp18"
        ) {
          throw new ERR_CRYPTO_UNKNOWN_DH_GROUP();
        }

        if (mode === kSync) {
          return op_node_generate_dh_group_key(group);
        } else {
          return op_node_generate_dh_group_key_async(group);
        }
      }

      if (prime != null) {
        if (primeLength != null) {
          throw new ERR_INCOMPATIBLE_OPTION_PAIR("prime", "primeLength");
        }

        validateBuffer(prime, "options.prime");
      } else if (primeLength != null) {
        validateInt32(primeLength, "options.primeLength", 0);
      } else {
        throw new ERR_MISSING_OPTION(
          "At least one of the group, prime, or primeLength options",
        );
      }

      if (generator != null) {
        validateInt32(generator, "options.generator", 0);
      }

      const g = generator == null ? 2 : generator;

      if (mode === kSync) {
        return op_node_generate_dh_key(prime, primeLength ?? 0, g);
      } else {
        return op_node_generate_dh_key_async(
          prime,
          primeLength ?? 0,
          g,
        );
      }
    }
    default:
      // Fall through
  }
  throw new ERR_INVALID_ARG_VALUE("type", type, "must be a supported key type");
}

return {
  generateKey,
  generateKeySync,
  generateKeyPair,
  generateKeyPairSync,
  default: {
    generateKey,
    generateKeySync,
    generateKeyPair,
    generateKeyPairSync,
  },
};
})();
