// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Crypto */
declare var crypto: Crypto;

/** @category Crypto */
interface Algorithm {
  name: string;
}

/** @category Crypto */
interface KeyAlgorithm {
  name: string;
}

/** @category Crypto */
type AlgorithmIdentifier = string | Algorithm;
/** @category Crypto */
type HashAlgorithmIdentifier = AlgorithmIdentifier;
/** @category Crypto */
type KeyType = "private" | "public" | "secret";
/** @category Crypto */
type KeyUsage =
  | "decrypt"
  | "deriveBits"
  | "deriveKey"
  | "encrypt"
  | "sign"
  | "unwrapKey"
  | "verify"
  | "wrapKey";
/** @category Crypto */
type KeyFormat = "jwk" | "pkcs8" | "raw" | "spki";
/** @category Crypto */
type NamedCurve = string;

/** @category Crypto */
interface RsaOtherPrimesInfo {
  d?: string;
  r?: string;
  t?: string;
}

/** @category Crypto */
interface JsonWebKey {
  alg?: string;
  crv?: string;
  d?: string;
  dp?: string;
  dq?: string;
  e?: string;
  ext?: boolean;
  k?: string;
  // deno-lint-ignore camelcase
  key_ops?: string[];
  kty?: string;
  n?: string;
  oth?: RsaOtherPrimesInfo[];
  p?: string;
  q?: string;
  qi?: string;
  use?: string;
  x?: string;
  y?: string;
}

/** @category Crypto */
interface AesCbcParams extends Algorithm {
  iv: BufferSource;
}

/** @category Crypto */
interface AesGcmParams extends Algorithm {
  iv: BufferSource;
  additionalData?: BufferSource;
  tagLength?: number;
}

/** @category Crypto */
interface AesCtrParams extends Algorithm {
  counter: BufferSource;
  length: number;
}

/** @category Crypto */
interface HmacKeyGenParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  length?: number;
}

/** @category Crypto */
interface EcKeyGenParams extends Algorithm {
  namedCurve: NamedCurve;
}

/** @category Crypto */
interface EcKeyImportParams extends Algorithm {
  namedCurve: NamedCurve;
}

/** @category Crypto */
interface EcdsaParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
}

/** @category Crypto */
interface RsaHashedImportParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
}

/** @category Crypto */
interface RsaHashedKeyGenParams extends RsaKeyGenParams {
  hash: HashAlgorithmIdentifier;
}

/** @category Crypto */
interface RsaKeyGenParams extends Algorithm {
  modulusLength: number;
  publicExponent: Uint8Array;
}

/** @category Crypto */
interface RsaPssParams extends Algorithm {
  saltLength: number;
}

/** @category Crypto */
interface RsaOaepParams extends Algorithm {
  label?: Uint8Array;
}

/** @category Crypto */
interface HmacImportParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  length?: number;
}

/** @category Crypto */
interface EcKeyAlgorithm extends KeyAlgorithm {
  namedCurve: NamedCurve;
}

/** @category Crypto */
interface HmacKeyAlgorithm extends KeyAlgorithm {
  hash: KeyAlgorithm;
  length: number;
}

/** @category Crypto */
interface RsaHashedKeyAlgorithm extends RsaKeyAlgorithm {
  hash: KeyAlgorithm;
}

/** @category Crypto */
interface RsaKeyAlgorithm extends KeyAlgorithm {
  modulusLength: number;
  publicExponent: Uint8Array;
}

/** @category Crypto */
interface HkdfParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  info: BufferSource;
  salt: BufferSource;
}

/** @category Crypto */
interface Pbkdf2Params extends Algorithm {
  hash: HashAlgorithmIdentifier;
  iterations: number;
  salt: BufferSource;
}

/** @category Crypto */
interface AesDerivedKeyParams extends Algorithm {
  length: number;
}

/** @category Crypto */
interface EcdhKeyDeriveParams extends Algorithm {
  public: CryptoKey;
}

/** @category Crypto */
interface AesKeyGenParams extends Algorithm {
  length: number;
}

/** @category Crypto */
interface AesKeyAlgorithm extends KeyAlgorithm {
  length: number;
}

/** The CryptoKey dictionary of the Web Crypto API represents a cryptographic
 * key.
 *
 * @category Crypto
 */
interface CryptoKey {
  readonly algorithm: KeyAlgorithm;
  readonly extractable: boolean;
  readonly type: KeyType;
  readonly usages: KeyUsage[];
}

/** @category Crypto */
declare var CryptoKey: {
  readonly prototype: CryptoKey;
  new (): never;
};

/** The CryptoKeyPair dictionary of the Web Crypto API represents a key pair for
 * an asymmetric cryptography algorithm, also known as a public-key algorithm.
 *
 * @category Crypto
 */
interface CryptoKeyPair {
  privateKey: CryptoKey;
  publicKey: CryptoKey;
}

/** @category Crypto */
declare var CryptoKeyPair: {
  readonly prototype: CryptoKeyPair;
  new (): never;
};

/** This Web Crypto API interface provides a number of low-level cryptographic
 * functions. It is accessed via the Crypto.subtle properties available in a
 * window context (via globalThis.crypto).
 *
 * @category Crypto
 */
interface SubtleCrypto {
  /**
   * 
   * This method is used to generate a cryptographic key.
   * 
   * @example
   * ```ts
   * // RSA key generation
   * const key = await crypto.subtle.generateKey(
   *   {
   *     name: "RSA-OAEP",
   *     modulusLength: 4096,
   *     publicExponent: new Uint8Array([1, 0, 1]),
   *     hash: "SHA-256",
   *   },
   *   true,
   *   ["encrypt", "decrypt"]
   * );
   * ```
   * 
   * @example
   * ```ts
   * // Elliptic curve (ECDSA) key pair generation
   * const key = await crypto.subtle.generateKey(
   *   {
   *     name: "ECDSA",
   *     namedCurve: "P-384",
   *   },
   *   true,
   *   ["sign", "verify"]
   * );
   * ```
  *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/generateKey
   */
  generateKey(
    algorithm: RsaHashedKeyGenParams | EcKeyGenParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKeyPair>;
  /**
  * 
   This method is used to generate a cryptographic key.
   * 
   * 
   * @example
   * ```ts
   * const key = await crypto.subtle.generateKey(
   *  {
   *    name: "AES-GCM",
   *    length: 256,
   *  },
   *  true,
   *  ["encrypt", "decrypt"]
   * );
   * ```
   * 
   * @example
   * ```ts
   * // HMAC key generation
   * const key = await crypto.subtle.generateKey(
   *  {
   *    name: "HMAC",
   *    hash: { name: "SHA-512" },
   *  },
   *  true,
   *  ["sign", "verify"]
   * );
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/generateKey
   */
  generateKey(
    algorithm: AesKeyGenParams | HmacKeyGenParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  /**
   * 
   * This method is used to generate a cryptographic key.
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/generateKey
   */
  generateKey(
    algorithm: AlgorithmIdentifier,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKeyPair | CryptoKey>;
  /**
   * 
   * This method is used to import a cryptographic key. 
   * The JWK (JSON Web Key format) imports an ECDSA private signing key, given a JSON Web Key object that represents it.
   * 
   *  @syntax
   * ```ts
   * importKey(format, keyData, algorithm, extractable, keyUsages)
   * ```
   * 
   * @param format The format of the key data. ["raw" | "spki" | "pkcs8" | "jwk"]
   * @param keyData The key data to import.
   * @param algorithm An object defining the type of key to import and providing extra parameters. 
   * @param extractable Whether the key is extractable.
   * @param keyUsages The key usages.
   *   * 
   * @example
   * ```ts  
   * // Import an ECDSA private signing key where `jwk` is an object describing a private key
   * function importKey(jwk) {
   *  return crypto.subtle.importKey(
   *    "jwk",
   *    jwk,
   *    {
   *      name: "ECDSA",  
   *      namedCurve: "P-384",
   *    },
   *    true,
   *    ["sign"],
   *  );
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/importKey
   */
  importKey(
    format: "jwk",
    keyData: JsonWebKey,
    algorithm:
      | AlgorithmIdentifier
      | HmacImportParams
      | RsaHashedImportParams
      | EcKeyImportParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  /**
   * 
   * This method is used to import a cryptographic key. 
   * 
   * @syntax
   * ```ts
   * importKey(format, keyData, algorithm, extractable, keyUsages)
   * ```
   * 
   * @param format The format of the key data. ["raw" | "spki" | "pkcs8" | "jwk"]
   * @param keyData The key data to import.
   * @param algorithm An object defining the type of key to import and providing extra parameters. 
   * @param extractable Whether the key is extractable.
   * @param keyUsages The key usages.
   *  
   * @example
   * ```ts
   * // Import an AES-GCM secret key where `rawKey` is an ArrayBuffer Sring
   * function importSecretKey(rawKey) { 
   *  return crypto.subtle.importKey("raw", rawKey, "AES-GCM", true, [
   *    "encrypt",
   *    "decrypt",
   *  ]);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/importKey
   */
  importKey(
    format: Exclude<KeyFormat, "jwk">,
    keyData: BufferSource,
    algorithm:
      | AlgorithmIdentifier
      | HmacImportParams
      | RsaHashedImportParams
      | EcKeyImportParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  /**
   * 
   * This method is used to export a cryptographic key.
   * 
   * @syntax
   * ```ts
   * exportKey(format, key) 
   * ```
   * 
   * @param format The format of the key data. ["raw" | "spki" | "pkcs8" | "jwk"]
   * @param key The key to export.
   * 
   * When the format specified is `jwk` the function returns a Promise which fulfils with a JSON object containing the key.
   *  
   * @example
   * ```ts
   * // Export a private key in JWK format
   * function exportKey(key) {
   *  // key is a CryptoKey object
   *  return await crypto.subtle.exportKey("jwk", key);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/exportKey
   * 
   */
  exportKey(format: "jwk", key: CryptoKey): Promise<JsonWebKey>;
  /**
   * 
   * This method is used to export a cryptographic key.
   * 
   * @syntax
   * ```ts
   * exportKey(format, key) 
   * ```
   * 
   * @param format The format of the key data. ["raw" | "spki" | "pkcs8" | "jwk"]
   * @param key The key to export.
   * 
   * When the format specified is other than `jwk` the function returns a Promise which fulfils with an ArrayBuffer containing the key.
   *  
   * @example
   * ```ts
   * // Export a private key in raw format
   * function exportKey(key) {
   *  // key is a CryptoKey object
   *  return await crypto.subtle.exportKey("raw", key);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/exportKey
   * 
   */
  exportKey(
    format: Exclude<KeyFormat, "jwk">,
    key: CryptoKey,
  ): Promise<ArrayBuffer>;
  /**
   *
   * This method is used to sign data using a cryptographic key and provide a digital signature.
   * 
   * @syntax
   * ```ts
   * sign(algorithm, key, data)
   * ```
   * 
   * @param algorithm The algorithm to use for the signing.
   * @param key The key to use for the signing.
   * @param data The data to sign.
   * 
   * @example
   * ```ts
   * // Sign data using an ECDSA private key
   * function signData(key, data) {
   *  return await crypto.subtle.sign("ECDSA", key, data);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/sign
   */
  sign(
    algorithm: AlgorithmIdentifier | RsaPssParams | EcdsaParams,
    key: CryptoKey,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  /**
   * 
   * This method is used to verify a digital signature using a cryptographic key and returns a boolean value.
   * 
   * @syntax
   * ```ts
   * verify(algorithm, key, signature, data)
   * ```
   * 
   * @param algorithm The algorithm to use for the verification.
   * @param key The key to use for the verification.
   * @param signature The signature to verify.
   * @param data The data to verify.
   * 
   * @example
   * ```ts
   * // Verify a digital signature using an ECDSA public key
   * function verifySignature(key, signature, data) {
   *  return await crypto.subtle.verify("ECDSA", key, signature, data);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/verify
   * 
   */
  verify(
    algorithm: AlgorithmIdentifier | RsaPssParams | EcdsaParams,
    key: CryptoKey,
    signature: BufferSource,
    data: BufferSource,
  ): Promise<boolean>;
  /**
   * 
   * This method is used to compute the digest of a given data using a cryptographic algorithm.
   * 
   * @syntax
   * ```ts
   * digest(algorithm, data)
   * ```
   * 
   * @param algorithm The algorithm to use for the digest. ["SHA-1" | "SHA-256" | "SHA-384" | "SHA-512"]
   * @param data The data to digest.
   * 
   * @example
   * ```ts
   * // Compute the digest of a given data using a cryptographic algorithm    
   * function computeDigest(data) {
   *  return await crypto.subtle.digest("SHA-256", data);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/digest
   */
  digest(
    algorithm: AlgorithmIdentifier,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  /**
   * 
   * This method is used to encrypt data using a cryptographic key and provide a ciphertext.
   * 
   * @syntax
   * ```ts
   * encrypt(algorithm, key, data)
   * ```
   * 
   * @param algorithm The algorithm to use for the encryption. ["RSA-OAEP" | "AES-CBC" | "AES-GCM" | "AES-CTR"]
   * @param key The key to use for the encryption.
   * @param data The data to encrypt.
   * 
   * @example
   * ```ts
   * // Encrypt data using an RSA-OAEP key  
   * function encryptData(key, data) {
   *  return await crypto.subtle.encrypt("RSA-OAEP", key, data);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/encrypt
   */
  encrypt(
    algorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCbcParams
      | AesGcmParams
      | AesCtrParams,
    key: CryptoKey,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  /**
   * 
   * This method is used to decrypt data using a cryptographic key and return the original plaintext.
   * 
   * @syntax
   * ```ts
   * decrypt(algorithm, key, data)
   * ```
   *  
   * @param algorithm : The algorithm to use for the decryption. ["RSA-OAEP" | "AES-CBC" | "AES-GCM" | "AES-CTR"]
   * @param key : The key to use for the decryption.
   * @param data : The data to decrypt.
   * 
   * @example
   * ```ts
   * // Decrypt data using an RSA-OAEP key
   * function decryptData(key, data) {  
   *   return await crypto.subtle.decrypt("RSA-OAEP", key, data);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/decrypt
   */
  decrypt(
    algorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCbcParams
      | AesGcmParams
      | AesCtrParams,
    key: CryptoKey,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  /**
   * 
   * This method is used to derive a key from a base key using a cryptographic algorithm.
   * 
   * @syntax
   * ```ts
   * deriveBits(algorithm, baseKey, length)
   * ```
   * 
   * @param algorithm The algorithm to use for the derivation. ["HKDF" | "PBKDF2" | "ECDH", "X25519"]
   * @param baseKey The base key to derive from.
   * @param length The length of the derived key.
   * 
   * @example
   * ```ts
   * // Derive a key using an HKDF algorithm
   * function deriveKey(baseKey, length) {
   *  return await crypto.subtle.deriveBits("HKDF", baseKey, length);
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/deriveBits
   */
  deriveBits(
    algorithm:
      | AlgorithmIdentifier
      | HkdfParams
      | Pbkdf2Params
      | EcdhKeyDeriveParams,
    baseKey: CryptoKey,
    length: number,
  ): Promise<ArrayBuffer>;
/**
 * 
 * This method is used to derive a secret key from a base or master key using a cryptographic algorithm.
 * It returns a Promise which fullfils with an object of the new key.
 * 
 * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/deriveKey
 * 
 * @syntax
 * ```ts
 * deriveKey(algorithm, baseKey, derivedKeyType, extractable, keyUsages)
 * ```
 * 
 * @example
 * ```ts
 * // Derive a key using an HKDF algorithm
 * function deriveKey(baseKey, derivedKeyType, extractable, keyUsages) {
 *  return await crypto.subtle.deriveKey("HKDF", baseKey, derivedKeyType, extractable, keyUsages);
 * }
 * ```
 * 
 * @param algorithm The algorithm to use for the derivation. ["HKDF" | "PBKDF2" | "ECDH", "X25519"]
 * @param baseKey The base key to derive from.
 * @param derivedKeyType The type of key to derive.
 * @param extractable Whether the key is extractable.
 * @param keyUsages An Array describing what the derived key can be used for.
 * 
 * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/deriveKey
 * 
 */
  deriveKey(
    algorithm:
      | AlgorithmIdentifier
      | HkdfParams
      | Pbkdf2Params
      | EcdhKeyDeriveParams,
    baseKey: CryptoKey,
    derivedKeyType:
      | AlgorithmIdentifier
      | AesDerivedKeyParams
      | HmacImportParams
      | HkdfParams
      | Pbkdf2Params,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  /**
   * 
   * The wrapKey() method "wraps" a key, which involves exporting it in a portable format and then encrypting that exported key.
   * Key wrapping enhances protection in untrusted environments, such as within an unsecured data store or during transmission over an unprotected network.
   * 
   * @syntax
   * ```ts
   * wrapKey(format, key, wrappingKey, wrapAlgorithm)
   * ```
   * 
   * @param format The format of the key data. ["raw" | "spki" | "pkcs8" | "jwk"]
   * @param key The key to wrap.
   * @param wrappingKey The key to use for the wrapping.
   * @param wrapAlgorithm The algorithm to use for the wrapping. ["RSA-OAEP" | "AES-CBC" | "AES-GCM" | "AES-CTR"]
   * 
   * @example
   * ```ts
   * // Wrap a key using an RSA-OAEP key
   * function wrapKey(key, wrappingKey) {
   *  return await crypto.subtle.wrapKey("jwk", key, wrappingKey, "RSA-OAEP");
   * }
   * ```
   * 
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/wrapKey
   */
  wrapKey(
    format: KeyFormat,
    key: CryptoKey,
    wrappingKey: CryptoKey,
    wrapAlgorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCbcParams
      | AesCtrParams,
  ): Promise<ArrayBuffer>;
  unwrapKey(
    format: KeyFormat,
    wrappedKey: BufferSource,
    unwrappingKey: CryptoKey,
    unwrapAlgorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCbcParams
      | AesCtrParams,
    unwrappedKeyAlgorithm:
      | AlgorithmIdentifier
      | HmacImportParams
      | RsaHashedImportParams
      | EcKeyImportParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
}

/** @category Crypto */
declare var SubtleCrypto: {
  readonly prototype: SubtleCrypto;
  new (): never;
};

/** @category Crypto */
interface Crypto {
  readonly subtle: SubtleCrypto;
  getRandomValues<
    T extends
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | BigInt64Array
      | BigUint64Array,
  >(
    array: T,
  ): T;
  randomUUID(): `${string}-${string}-${string}-${string}-${string}`;
}

/** @category Crypto */
declare var Crypto: {
  readonly prototype: Crypto;
  new (): never;
};
