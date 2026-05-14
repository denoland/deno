// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

(function () {
const { core } = globalThis.__bootstrap;
const { ERR_CRYPTO_FIPS_FORCED } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const { crypto: constants } = core.loadExtScript(
  "ext:deno_node/internal_binding/constants.ts",
);
const { getOptionValue } = core.loadExtScript(
  "ext:deno_node/internal/options.ts",
);
const {
  getFipsCrypto,
  setFipsCrypto,
  timingSafeEqual,
} = core.loadExtScript("ext:deno_node/internal_binding/crypto.ts");
const {
  checkPrime,
  checkPrimeSync,
  generatePrime,
  generatePrimeSync,
  randomBytes,
  randomFill,
  randomFillSync,
  randomInt,
  randomUUID,
} = core.loadExtScript("ext:deno_node/internal/crypto/random.ts");
const { pbkdf2, pbkdf2Sync } = core.loadExtScript(
  "ext:deno_node/internal/crypto/pbkdf2.ts",
);
const { scrypt, scryptSync } = core.loadExtScript(
  "ext:deno_node/internal/crypto/scrypt.ts",
);
const { hkdf, hkdfSync } = core.loadExtScript(
  "ext:deno_node/internal/crypto/hkdf.ts",
);
const {
  generateKey,
  generateKeyPair,
  generateKeyPairSync,
  generateKeySync,
} = core.loadExtScript("ext:deno_node/internal/crypto/keygen.ts");
const {
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  KeyObject,
} = core.loadExtScript("ext:deno_node/internal/crypto/keys.ts");
const {
  DiffieHellman,
  diffieHellman,
  DiffieHellmanGroup,
  ECDH,
} = core.loadExtScript("ext:deno_node/internal/crypto/diffiehellman.ts");
const {
  Cipheriv,
  Decipheriv,
  privateDecrypt,
  privateEncrypt,
  publicDecrypt,
  publicEncrypt,
} = core.loadExtScript("ext:deno_node/internal/crypto/cipher.ts");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  Sign,
  signOneShot,
  Verify,
  verifyOneShot,
} = core.loadExtScript("ext:deno_node/internal/crypto/sig.ts");
const {
  createHash,
  getHashes,
  Hash: Hash_,
  Hmac: Hmac_,
} = core.loadExtScript("ext:deno_node/internal/crypto/hash.ts");
const { X509Certificate } = core.loadExtScript(
  "ext:deno_node/internal/crypto/x509.ts",
);
const {
  getCipherInfo,
  getCiphers,
  getCurves,
  secureHeapUsed,
  setEngine,
} = core.loadExtScript("ext:deno_node/internal/crypto/util.ts");
const { default: Certificate } = core.loadExtScript(
  "ext:deno_node/internal/crypto/certificate.ts",
);
const { normalizeEncoding } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
const { isArrayBufferView } = core.loadExtScript(
  "ext:deno_node/internal/util/types.ts",
);
const { validateString } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { crypto: webcrypto } = core.loadExtScript(
  "ext:deno_crypto/00_crypto.js",
);
const { deprecate } = core.loadExtScript("ext:deno_node/util.ts");

const subtle = webcrypto.subtle;
const fipsForced = getOptionValue("--force-fips");

const Hash = deprecate(
  Hash_,
  "crypto.Hash constructor is deprecated.",
  "DEP0179",
);
const Hmac = deprecate(
  Hmac_,
  "crypto.Hmac constructor is deprecated.",
  "DEP0181",
);

function getRandomValues(typedArray) {
  return webcrypto.getRandomValues(typedArray);
}

function hash(
  algorithm: string,
  data: BinaryLike,
  outputEncodingOrOptions: BinaryToTextEncoding | {
    outputEncoding?: BinaryToTextEncoding;
    outputLength?: number;
  } = "hex",
) {
  validateString(algorithm, "algorithm");
  if (typeof data !== "string" && !isArrayBufferView(data)) {
    throw new ERR_INVALID_ARG_TYPE("input", [
      "Buffer",
      "TypedArray",
      "DataView",
      "string",
    ], data);
  }

  let outputEncoding: string;
  let outputLength: number | undefined;

  if (typeof outputEncodingOrOptions === "object") {
    outputEncoding = outputEncodingOrOptions.outputEncoding ?? "hex";
    outputLength = outputEncodingOrOptions.outputLength;
  } else {
    outputEncoding = outputEncodingOrOptions;
  }

  let normalized = outputEncoding;
  // Fast case: if it's 'hex', we don't need to validate it further.
  if (outputEncoding !== "hex") {
    validateString(outputEncoding, "outputEncoding");
    normalized = normalizeEncoding(outputEncoding);
    // If the encoding is invalid, normalizeEncoding() returns undefined.
    if (normalized === undefined) {
      // normalizeEncoding() doesn't handle 'buffer'.
      if (outputEncoding.toLowerCase() === "buffer") {
        normalized = "buffer";
      } else {
        throw new ERR_INVALID_ARG_VALUE("outputEncoding", outputEncoding);
      }
    }
  }

  const algoLower = algorithm.toLowerCase();
  const isXof = algoLower === "shake128" || algoLower === "shake256";

  if (outputLength != null && !isXof) {
    // For non-XOF hashes, outputLength must match the algorithm's digest size.
    const testHash = createHash(algorithm);
    testHash.update("");
    const expectedLen = testHash.digest().length;
    if (outputLength !== expectedLen) {
      throw new Error(
        `Output length ${outputLength} is invalid for ${algoLower}, which does not support XOF`,
      );
    }
  }

  const h = createHash(
    algorithm,
    outputLength != null ? { outputLength } : undefined,
  );
  h.update(data);

  if (outputLength === 0) {
    return normalized === "buffer" ? globalThis.Buffer.alloc(0) : "";
  }

  return h.digest(outputEncoding);
}

function validateCipherivArgs(
  cipher: unknown,
  key: unknown,
  iv: unknown,
) {
  if (typeof cipher !== "string") {
    throw new ERR_INVALID_ARG_TYPE(
      "cipher",
      "string",
      cipher,
    );
  }
  if (
    typeof key !== "string" && !isArrayBufferView(key) &&
    !(key && typeof key === "object" && "type" in key)
  ) {
    throw new ERR_INVALID_ARG_TYPE(
      "key",
      ["string", "ArrayBufferView", "Buffer", "KeyObject"],
      key,
    );
  }
  if (
    iv !== null && typeof iv !== "string" && !isArrayBufferView(iv)
  ) {
    throw new ERR_INVALID_ARG_TYPE(
      "iv",
      ["string", "ArrayBufferView", "Buffer", "null"],
      iv,
    );
  }
}

function createCipheriv(
  algorithm: CipherCCMTypes,
  key: CipherKey,
  iv: BinaryLike,
  options: CipherCCMOptions,
): CipherCCM;
function createCipheriv(
  algorithm: CipherOCBTypes,
  key: CipherKey,
  iv: BinaryLike,
  options: CipherOCBOptions,
): CipherOCB;
function createCipheriv(
  algorithm: CipherGCMTypes,
  key: CipherKey,
  iv: BinaryLike,
  options?: CipherGCMOptions,
): CipherGCM;
function createCipheriv(
  algorithm: string,
  key: CipherKey,
  iv: BinaryLike | null,
  options?: TransformOptions,
): Cipher;
function createCipheriv(
  cipher: string,
  key: CipherKey,
  iv: BinaryLike | null,
  options?: TransformOptions,
): Cipher {
  validateCipherivArgs(cipher, key, iv);
  return Cipheriv(cipher, key, iv, options);
}

function createDecipheriv(
  algorithm: CipherCCMTypes,
  key: CipherKey,
  iv: BinaryLike,
  options: CipherCCMOptions,
): DecipherCCM;
function createDecipheriv(
  algorithm: CipherOCBTypes,
  key: CipherKey,
  iv: BinaryLike,
  options: CipherOCBOptions,
): DecipherOCB;
function createDecipheriv(
  algorithm: CipherGCMTypes,
  key: CipherKey,
  iv: BinaryLike,
  options?: CipherGCMOptions,
): DecipherGCM;
function createDecipheriv(
  algorithm: string,
  key: CipherKey,
  iv: BinaryLike | null,
  options?: TransformOptions,
): Decipher {
  validateCipherivArgs(algorithm, key, iv);
  return Decipheriv(algorithm, key, iv, options);
}

function createDiffieHellman(
  primeLength: number,
  generator?: number | ArrayBufferView,
): DiffieHellman;
function createDiffieHellman(prime: ArrayBufferView): DiffieHellman;
function createDiffieHellman(
  prime: string,
  primeEncoding: BinaryToTextEncoding,
): DiffieHellman;
function createDiffieHellman(
  prime: string,
  primeEncoding: BinaryToTextEncoding,
  generator: number | ArrayBufferView,
): DiffieHellman;
function createDiffieHellman(
  prime: string,
  primeEncoding: BinaryToTextEncoding,
  generator: string,
  generatorEncoding: BinaryToTextEncoding,
): DiffieHellman;
function createDiffieHellman(
  sizeOrKey: number | string | ArrayBufferView,
  keyEncoding?: number | ArrayBufferView | BinaryToTextEncoding,
  generator?: number | ArrayBufferView | string,
  generatorEncoding?: BinaryToTextEncoding,
): DiffieHellman {
  return new DiffieHellman(
    sizeOrKey,
    keyEncoding,
    generator,
    generatorEncoding,
  );
}

function createDiffieHellmanGroup(name: string): DiffieHellmanGroup {
  return new DiffieHellmanGroup(name);
}

function createECDH(curve: string): ECDH {
  return new ECDH(curve);
}

function createHmac(
  hmac: string,
  key: string | ArrayBuffer | KeyObject,
  options?: TransformOptions,
) {
  return Hmac_(hmac, key, options);
}

function createSign(algorithm: string, options?: WritableOptions): Sign {
  return new Sign(algorithm, options);
}

function createVerify(algorithm: string, options?: WritableOptions): Verify {
  return new Verify(algorithm, options);
}

function setFipsForced(val: boolean) {
  if (val) {
    return;
  }

  throw new ERR_CRYPTO_FIPS_FORCED();
}

function getFipsForced() {
  return 1;
}

Object.defineProperty(constants, "defaultCipherList", {
  value: getOptionValue("--tls-cipher-list"),
});

const getDiffieHellman = createDiffieHellmanGroup;

const getFips = fipsForced ? getFipsForced : getFipsCrypto;
const setFips = fipsForced ? setFipsForced : setFipsCrypto;

const sign = signOneShot;
const verify = verifyOneShot;

/* Deprecated in Node.js, alias of randomBytes */
const pseudoRandomBytes = randomBytes;

const defaultExport = {
  Certificate,
  checkPrime,
  checkPrimeSync,
  Cipheriv,
  constants,
  createCipheriv,
  createDecipheriv,
  createDiffieHellman,
  createDiffieHellmanGroup,
  createECDH,
  createHash,
  createHmac,
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  createSign,
  createVerify,
  Decipheriv,
  DiffieHellman,
  diffieHellman,
  DiffieHellmanGroup,
  ECDH,
  getRandomValues,
  generateKey,
  generateKeyPair,
  generateKeyPairSync,
  generateKeySync,
  generatePrime,
  generatePrimeSync,
  getCipherInfo,
  getCiphers,
  getCurves,
  getDiffieHellman,
  getFips,
  getHashes,
  hash,
  Hash,
  hkdf,
  hkdfSync,
  Hmac,
  KeyObject,
  pbkdf2,
  pbkdf2Sync,
  privateDecrypt,
  privateEncrypt,
  publicDecrypt,
  publicEncrypt,
  randomBytes,
  randomFill,
  randomFillSync,
  randomInt,
  randomUUID,
  scrypt,
  scryptSync,
  secureHeapUsed,
  setEngine,
  setFips,
  Sign,
  sign,
  timingSafeEqual,
  Verify,
  verify,
  webcrypto,
  subtle,
  X509Certificate,
};

// Aliases for randomBytes are deprecated; defined as non-enumerable lazy
// getters to mirror Node's lib/crypto.js getRandomBytesAlias(). With
// --pending-deprecation, accessing them prints DEP0115.
function defineRandomBytesAlias(target: object, key: string) {
  Object.defineProperty(target, key, {
    enumerable: false,
    configurable: true,
    get() {
      const value = getOptionValue("--pending-deprecation")
        ? deprecate(randomBytes, `crypto.${key} is deprecated.`, "DEP0115")
        : randomBytes;
      Object.defineProperty(this, key, {
        enumerable: false,
        configurable: true,
        writable: true,
        value,
      });
      return value;
    },
    set(value) {
      Object.defineProperty(this, key, {
        enumerable: false,
        configurable: true,
        writable: true,
        value,
      });
    },
  });
}
for (const key of ["pseudoRandomBytes", "prng", "rng"]) {
  defineRandomBytesAlias(defaultExport, key);
}

return {
  default: defaultExport,
  Certificate,
  checkPrime,
  checkPrimeSync,
  Cipheriv,
  constants,
  createCipheriv,
  createDecipheriv,
  createDiffieHellman,
  createDiffieHellmanGroup,
  createECDH,
  createHash,
  createHmac,
  createPrivateKey,
  createPublicKey,
  createSecretKey,
  createSign,
  createVerify,
  Decipheriv,
  DiffieHellman,
  diffieHellman,
  DiffieHellmanGroup,
  ECDH,
  generateKey,
  generateKeyPair,
  generateKeyPairSync,
  generateKeySync,
  generatePrime,
  generatePrimeSync,
  getCipherInfo,
  getCiphers,
  getCurves,
  getDiffieHellman,
  getFips,
  getHashes,
  getRandomValues,
  Hash,
  hash,
  hkdf,
  hkdfSync,
  Hmac,
  KeyObject,
  pbkdf2,
  pbkdf2Sync,
  privateDecrypt,
  privateEncrypt,
  /* Deprecated in Node.js, alias of randomBytes */
  pseudoRandomBytes,
  publicDecrypt,
  publicEncrypt,
  randomBytes,
  randomFill,
  randomFillSync,
  randomInt,
  randomUUID,
  scrypt,
  scryptSync,
  secureHeapUsed,
  setEngine,
  setFips,
  Sign,
  sign,
  subtle,
  timingSafeEqual,
  Verify,
  verify,
  webcrypto,
  X509Certificate,
};
})();
