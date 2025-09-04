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
   * Generates an asymmetric cryptographic key pair for encryption, signing, or
   * key exchange.
   *
   * This overload is used for generating key pairs with RSA or elliptic curve
   * algorithms.
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
   * Generates a symmetric cryptographic key for encryption, authentication, or
   * hashing.
   *
   * This overload is used for algorithms such as AES and HMAC.
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
   * Generates a cryptographic key or key pair for a given algorithm.
   *
   * This generic overload handles any key generation request, returning either
   * a symmetric key or an asymmetric key pair based on the provided algorithm.
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/generateKey
   */
  generateKey(
    algorithm: AlgorithmIdentifier,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKeyPair | CryptoKey>;

  /**
   * Imports a cryptographic key in JSON Web Key (JWK) format.
   *
   * This method is used to import an asymmetric key (e.g., RSA or ECDSA) from a JWK object.
   * JWK allows structured representation of keys, making them portable across different systems.
   *
   * @example
   * ```ts
   * // Import an ECDSA private signing key where `jwk` is an object describing a private key
   * crypto.subtle.importKey(
   *   "jwk",
   *   jwk,
   *   {
   *     name: "ECDSA",
   *     namedCurve: "P-384",
   *   },
   *   true,
   *   ["sign"],
   * );
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
   * Imports a cryptographic key in raw, PKCS8, or SPKI format.
   *
   * This method is used to import symmetric keys (e.g., AES), private keys (PKCS8), or public keys (SPKI).
   *
   * @example
   * ```ts
   * // Import an AES-GCM secret key where `rawKey` is an ArrayBuffer string
   * crypto.subtle.importKey("raw", rawKey, "AES-GCM", true, [
   *   "encrypt",
   *   "decrypt",
   * ]);
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
   * Exports a cryptographic key in JSON Web Key (JWK) format.
   *
   * This method allows exporting an asymmetric key (e.g., RSA, ECDSA) into a JSON-based representation,
   * making it easy to store and transfer across systems.
   *
   * @example
   * ```ts
   * await crypto.subtle.exportKey("jwk", key);
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/exportKey
   */
  exportKey(format: "jwk", key: CryptoKey): Promise<JsonWebKey>;
  /**
   * Exports a cryptographic key in raw, PKCS8, or SPKI format.
   *
   * This method is used to export symmetric keys (AES), private keys (PKCS8), or public keys (SPKI) in binary form.
   *
   * @example
   * ```ts
   * await crypto.subtle.exportKey("raw", key);
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/exportKey
   */
  exportKey(
    format: Exclude<KeyFormat, "jwk">,
    key: CryptoKey,
  ): Promise<ArrayBuffer>;
  /**
   * Generates a digital signature using a private cryptographic key.
   *
   * This method is used to sign data with an asymmetric key (e.g., RSA-PSS, ECDSA).
   *
   * @example
   * ```ts
   * await crypto.subtle.sign("ECDSA", key, data);
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
   * Verifies a digital signature using a public cryptographic key.
   *
   * This method checks whether a signature is valid for the given data.
   *
   * @example
   * ```ts
   * await crypto.subtle.verify("ECDSA", key, signature, data);
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/verify
   */
  verify(
    algorithm: AlgorithmIdentifier | RsaPssParams | EcdsaParams,
    key: CryptoKey,
    signature: BufferSource,
    data: BufferSource,
  ): Promise<boolean>;
  /**
   * Computes a cryptographic hash (digest) of the given data.
   *
   * This method is commonly used for verifying data integrity.
   *
   * @example
   * ```ts
   * // Compute the digest of given data using a cryptographic algorithm
   * await crypto.subtle.digest("SHA-256", data);
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/digest
   */
  digest(
    algorithm: AlgorithmIdentifier,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  /**
   * Encrypts data using a cryptographic key.
   *
   * This method is used with both symmetric (AES) and asymmetric (RSA) encryption.
   *
   * @example
   * ```ts
   * await crypto.subtle.encrypt("RSA-OAEP", key, data);
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
   * Decrypts previously encrypted data using a cryptographic key.
   *
   * @example
   * ```ts
   * await crypto.subtle.decrypt("RSA-OAEP", key, data);
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
   * This method is used to derive a key from a base key using a cryptographic algorithm.
   *
   * @example
   * ```ts
   * await crypto.subtle.deriveBits("HKDF", baseKey, length);
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
   * This method is used to derive a secret key from a base or master key using a cryptographic algorithm.
   * It returns a Promise which fulfils with an object of the new key.
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/deriveKey
   *
   * @example
   * ```ts
   * // Derive a key using an HKDF algorithm
   * await crypto.subtle.deriveKey("HKDF", baseKey, derivedKeyType, extractable, keyUsages);
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/deriveKey
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
   * Wraps (encrypts) a cryptographic key for secure storage or transmission
   *
   * @example
   * ```ts
   * await crypto.subtle.wrapKey("jwk", key, wrappingKey, "RSA-OAEP");
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
  /**
   * Unwraps (decrypts) a previously wrapped key.
   *
   * @example
   * ```ts
   * // Unwrap an AES-GCM key wrapped with AES-KW
   * const unwrappedKey = await crypto.subtle.unwrapKey(
   *   "jwk", // Format of the key to import
   *   wrappedKey, // Encrypted key data as ArrayBuffer
   *   unwrappingKey, // CryptoKey used for unwrapping
   *   { name: "AES-KW" }, // Unwrapping algorithm
   *   { name: "AES-GCM", length: 256 }, // Algorithm for unwrapped key
   *   true, // Whether the unwrapped key is extractable
   *   ["encrypt", "decrypt"] // Allowed key usages
   * );
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto/unwrapKey
   */
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

  /**
   * Mutates the provided typed array with cryptographically secure random
   * values.
   *
   * @returns The same typed array, now populated with random values.
   *
   * @example
   * ```ts
   * const array = new Uint32Array(4);
   * crypto.getRandomValues(array);
   * console.log(array);
   * // output: Uint32Array(4) [ 3629234207, 1947236412, 3171234560, 4294901234 ]
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/Crypto/getRandomValues
   */
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

  /**
   * Generates a random RFC 4122 version 4 UUID using a cryptographically
   * secure random number generator.
   *
   * @returns A randomly generated, 36-character long v4 UUID.
   *
   * @example
   * ```ts
   * const uuid = crypto.randomUUID();
   * console.log(uuid);
   * // Example output: '36b8f84d-df4e-4d49-b662-bcde71a8764f'
   * ```
   *
   * The `randomUUID` method generates a version 4 UUID, which is purely
   * random. If you require other versions of UUIDs, such as time-based (v1) or
   * name-based (v3 and v5), consider using the `@std/uuid` package available
   * at {@link https://jsr.io/@std/uuid}.
   *
   * @example
   * ```ts
   * import { v1 } from 'jsr:@std/uuid';
   *
   * // Generate a time-based UUID (v1)
   * const uuidV1 = v1.generate();
   * console.log(uuidV1);
   * // output: 'a0c74f7e-82f1-11eb-8dcd-0242ac130003'
   * ```
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/Crypto/randomUUID
   */
  randomUUID(): `${string}-${string}-${string}-${string}-${string}`;
}

/** @category Crypto */
declare var Crypto: {
  readonly prototype: Crypto;
  new (): never;
};
